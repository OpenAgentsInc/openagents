use super::*;

impl Storybook {
    pub(crate) fn paint_thread_components(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let header_height = panel_height(160.0);
        let editor_height = panel_height(180.0);
        let feedback_height = panel_height(200.0);
        let actions_height = panel_height(140.0);
        let terminal_height = panel_height(140.0);
        let layout_height = panel_height(400.0);
        let trajectory_height = panel_height(220.0);

        let panels = panel_stack(
            bounds,
            &[
                header_height,
                editor_height,
                feedback_height,
                actions_height,
                terminal_height,
                layout_height,
                trajectory_height,
            ],
        );

        // ========== Panel 1: Thread Headers ==========
        let header_bounds = panels[0];
        draw_panel("Thread Headers", header_bounds, cx, |inner, cx| {
            let variants = [
                ("Full header", true, true, Some("3 messages")),
                ("No back button", false, true, None),
                ("No menu button", true, false, None),
                ("Minimal", false, false, Some("subtitle only")),
            ];

            let tile_w = 280.0;
            let tile_h = 60.0;
            let gap = 16.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, show_back, show_menu, subtitle)) in variants.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // ThreadHeader
                let mut header = ThreadHeader::new("Conversation")
                    .show_back_button(*show_back)
                    .show_menu_button(*show_menu);
                if let Some(sub) = subtitle {
                    header = header.subtitle(*sub);
                }
                header.paint(Bounds::new(tile_x, tile_y + 14.0, tile_w, 48.0), cx);
            }
        });

        // ========== Panel 2: Message Editor States ==========
        let editor_bounds = panels[1];
        draw_panel("Message Editor States", editor_bounds, cx, |inner, cx| {
            let states = [
                ("Normal mode", Mode::Normal, false, "Type a message..."),
                ("Plan mode", Mode::Plan, false, "Describe your plan..."),
                ("Streaming", Mode::Normal, true, ""),
            ];

            let tile_w = 320.0;
            let tile_h = 70.0;
            let gap = 16.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, mode, streaming, placeholder)) in states.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // MessageEditor
                let mut editor = MessageEditor::new().mode(*mode).streaming(*streaming);
                if !placeholder.is_empty() {
                    editor = editor.placeholder(*placeholder);
                }
                editor.paint(Bounds::new(tile_x, tile_y + 14.0, tile_w, 64.0), cx);
            }
        });

        // ========== Panel 3: Thread Feedback ==========
        let feedback_bounds = panels[2];
        draw_panel("Thread Feedback", feedback_bounds, cx, |inner, cx| {
            let tile_w = 280.0;
            let gap = 16.0;

            // Default state
            let label_run = cx.text.layout(
                "Default (no rating)",
                Point::new(inner.origin.x, inner.origin.y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run);

            let mut feedback1 = ThreadFeedback::new();
            feedback1.paint(
                Bounds::new(inner.origin.x, inner.origin.y + 14.0, tile_w, 80.0),
                cx,
            );

            // Second column - with comment shown (simulated by larger height)
            let label_run2 = cx.text.layout(
                "Rating selected",
                Point::new(inner.origin.x + tile_w + gap, inner.origin.y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run2);

            // Show a description of what would happen
            let info = cx.text.layout(
                "Click thumbs up/down to rate",
                Point::new(inner.origin.x + tile_w + gap, inner.origin.y + 50.0),
                theme::font_size::XS,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(info);
        });

        // ========== Panel 4: Entry Actions ==========
        let actions_bounds = panels[3];
        draw_panel("Entry Actions", actions_bounds, cx, |inner, cx| {
            let variants = [
                ("Default (copy)", true, false, false, false),
                ("With retry", true, true, false, false),
                ("With edit/delete", true, false, true, true),
                ("All actions", true, true, true, true),
                ("Copy only", true, false, false, false),
            ];

            let tile_w = 200.0;
            let tile_h = 45.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, copy, retry, edit, delete)) in variants.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // EntryActions
                let mut actions = EntryActions::new()
                    .show_copy(*copy)
                    .show_retry(*retry)
                    .show_edit(*edit)
                    .show_delete(*delete);
                actions.paint(Bounds::new(tile_x, tile_y + 16.0, tile_w, 24.0), cx);
            }
        });

        // ========== Panel 5: Terminal Headers ==========
        let terminal_bounds = panels[4];
        draw_panel("Terminal Headers", terminal_bounds, cx, |inner, cx| {
            let variants = [
                ("Pending", "cargo build", ToolStatus::Pending, None),
                ("Running", "npm install", ToolStatus::Running, None),
                ("Success", "cargo test", ToolStatus::Success, Some(0)),
                ("Error", "rm -rf /", ToolStatus::Error, Some(1)),
            ];

            let tile_w = 280.0;
            let tile_h = 45.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, cmd, status, exit_code)) in variants.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // TerminalHeader
                let mut header = TerminalHeader::new(*cmd).status(*status);
                if let Some(code) = exit_code {
                    header = header.exit_code(*code);
                }
                header.paint(Bounds::new(tile_x, tile_y + 14.0, tile_w, 32.0), cx);
            }
        });

        // ========== Panel 6: Complete Thread Layout ==========
        let layout_bounds = panels[5];
        draw_panel("Complete Thread Layout", layout_bounds, cx, |inner, cx| {
            // ThreadHeader at top
            let mut header = ThreadHeader::new("Code Review Session")
                .subtitle("5 messages")
                .show_back_button(true)
                .show_menu_button(true);
            header.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 48.0),
                cx,
            );

            // Thread content area
            let content_y = inner.origin.y + 56.0;
            let content_h = inner.size.height - 56.0 - 72.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    inner.origin.x,
                    content_y,
                    inner.size.width,
                    content_h,
                ))
                .with_background(theme::bg::APP)
                .with_border(theme::border::DEFAULT, 1.0),
            );

            // Sample messages
            let msg1 = cx.text.layout(
                "User: Can you review this code?",
                Point::new(inner.origin.x + 12.0, content_y + 12.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(msg1);

            let msg2 = cx.text.layout(
                "Assistant: I'll analyze the code structure...",
                Point::new(inner.origin.x + 12.0, content_y + 36.0),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(msg2);

            // Entry actions for a message
            let mut actions = EntryActions::new().show_copy(true).show_retry(true);
            actions.paint(
                Bounds::new(inner.origin.x + 12.0, content_y + 60.0, 180.0, 24.0),
                cx,
            );

            // Terminal header in content
            let mut terminal = TerminalHeader::new("cargo clippy")
                .status(ToolStatus::Success)
                .exit_code(0);
            terminal.paint(
                Bounds::new(inner.origin.x + 12.0, content_y + 92.0, 300.0, 32.0),
                cx,
            );

            // MessageEditor at bottom
            let editor_y = inner.origin.y + inner.size.height - 64.0;
            let mut editor = MessageEditor::new()
                .mode(Mode::Normal)
                .placeholder("Continue the conversation...");
            editor.paint(
                Bounds::new(inner.origin.x, editor_y, inner.size.width, 64.0),
                cx,
            );
        });

        // ========== Panel 7: Trajectory View ==========
        let trajectory_bounds = panels[6];
        draw_panel("Trajectory View", trajectory_bounds, cx, |inner, cx| {
            let entries = vec![
                TrajectoryEntry::new("Load workspace")
                    .detail("Open repository state")
                    .timestamp("00:12")
                    .status(TrajectoryStatus::Verified),
                TrajectoryEntry::new("Analyze failing tests")
                    .detail("Unit tests: 3 failed")
                    .timestamp("00:32")
                    .status(TrajectoryStatus::Partial),
                TrajectoryEntry::new("Apply fix")
                    .detail("Update parser edge cases")
                    .timestamp("01:05")
                    .status(TrajectoryStatus::Verified),
                TrajectoryEntry::new("Re-run suite")
                    .detail("All green")
                    .timestamp("01:42")
                    .status(TrajectoryStatus::Verified),
            ];

            let mut view = TrajectoryView::new().entries(entries);
            view.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width,
                    inner.size.height,
                ),
                cx,
            );
        });
    }
}
