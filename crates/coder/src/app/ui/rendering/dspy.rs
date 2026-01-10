fn render_dspy_stage_card(
    stage: &DspyStage,
    bounds: Bounds,
    cx: &mut PaintContext,
    palette: &UiPalette,
) {
    let padding = 12.0;
    let font_size = 13.0;
    let small_font_size = 11.0;
    let line_height = font_size * 1.4;
    let small_line_height = small_font_size * 1.4;

    // Background with accent border based on stage type
    let (header_text, accent_color, icon) = match stage {
        DspyStage::EnvironmentAssessment { .. } => (
            "Environment Assessment",
            Hsla::new(200.0 / 360.0, 0.7, 0.5, 1.0), // Blue
            "🔍",
        ),
        DspyStage::Planning { .. } => (
            "Planning",
            Hsla::new(280.0 / 360.0, 0.6, 0.5, 1.0), // Purple
            "📋",
        ),
        DspyStage::TodoList { .. } => (
            "Todo List",
            Hsla::new(120.0 / 360.0, 0.6, 0.45, 1.0), // Green
            "✅",
        ),
        DspyStage::ExecutingTask { .. } => (
            "Executing",
            Hsla::new(30.0 / 360.0, 0.8, 0.5, 1.0), // Orange
            "🔧",
        ),
        DspyStage::TaskComplete { .. } => (
            "Task Complete",
            Hsla::new(160.0 / 360.0, 0.6, 0.5, 1.0), // Teal
            "✅",
        ),
        DspyStage::Complete { .. } => (
            "Complete",
            Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0), // Green
            "🏁",
        ),
    };

    // Card background
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(palette.panel)
            .with_border(accent_color, 2.0)
            .with_corner_radius(8.0),
    );

    // Header
    let header_y = bounds.origin.y + padding;
    let icon_run = cx.text.layout_styled_mono(
        icon,
        Point::new(bounds.origin.x + padding, header_y),
        font_size,
        accent_color,
        wgpui::text::FontStyle::default(),
    );
    cx.scene.draw_text(icon_run);

    let header_run = cx.text.layout_styled_mono(
        header_text,
        Point::new(bounds.origin.x + padding + 20.0, header_y),
        font_size,
        palette.text_primary,
        wgpui::text::FontStyle::default(),
    );
    cx.scene.draw_text(header_run);

    // Content
    let content_x = bounds.origin.x + padding;
    let mut y = header_y + line_height + 8.0;

    match stage {
        DspyStage::EnvironmentAssessment {
            system_info,
            workspace,
            active_directive,
            open_issues,
            compute_backends,
            priority_action,
            urgency,
            reasoning,
        } => {
            let directive = active_directive.as_deref().unwrap_or("None");
            let backends = if compute_backends.is_empty() {
                "None".to_string()
            } else {
                compute_backends.join(", ")
            };
            let status_line = format!("{} open · backends: {}", open_issues, backends);
            let priority_line = format!("{} ({})", priority_action, urgency);
            let reasoning_line = truncate_preview(reasoning, 140);
            let mut items = vec![
                ("System".to_string(), truncate_preview(system_info, 120)),
                ("Workspace".to_string(), truncate_preview(workspace, 120)),
                ("Directive".to_string(), truncate_preview(directive, 120)),
                ("Status".to_string(), status_line),
                ("Priority".to_string(), priority_line),
            ];
            if !reasoning_line.is_empty() {
                items.push(("Reasoning".to_string(), reasoning_line));
            }
            for (label, text) in items {
                let line = format!("{}: {}", label, text);
                for wrapped in wrap_text(&line, 80) {
                    let run = cx.text.layout_styled_mono(
                        &wrapped,
                        Point::new(content_x, y),
                        small_font_size,
                        palette.text_primary,
                        wgpui::text::FontStyle::default(),
                    );
                    cx.scene.draw_text(run);
                    y += small_line_height;
                }
                y += 4.0;
            }
        }
        DspyStage::Planning {
            analysis,
            implementation_steps,
            test_strategy,
            complexity,
            confidence,
            ..
        } => {
            let analysis_line = format!("Analysis: {}", truncate_preview(analysis, 160));
            for line in wrap_text(&analysis_line, 80) {
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    small_font_size,
                    palette.text_primary,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += small_line_height;
            }
            y += 4.0;

            let complexity_line =
                format!("Complexity: {} ({:.0}%)", complexity, confidence * 100.0);
            let run = cx.text.layout_styled_mono(
                &complexity_line,
                Point::new(content_x, y),
                small_font_size,
                palette.text_dim,
                wgpui::text::FontStyle::default(),
            );
            cx.scene.draw_text(run);
            y += small_line_height + 4.0;

            let test_line = format!("Test: {}", truncate_preview(test_strategy, 160));
            for line in wrap_text(&test_line, 80) {
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    small_font_size,
                    palette.text_dim,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += small_line_height;
            }
            y += 6.0;

            for (i, step) in implementation_steps.iter().enumerate() {
                let line = format!("{}. {}", i + 1, step);
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    small_font_size,
                    palette.text_primary,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += small_line_height;
            }
        }
        DspyStage::TodoList { tasks } => {
            for task in tasks {
                let status_symbol = match task.status {
                    crate::autopilot_loop::TodoStatus::Pending => "□",
                    crate::autopilot_loop::TodoStatus::InProgress => "◐",
                    crate::autopilot_loop::TodoStatus::Complete => "✓",
                    crate::autopilot_loop::TodoStatus::Failed => "✗",
                };
                let color = match task.status {
                    crate::autopilot_loop::TodoStatus::Pending => palette.text_dim,
                    crate::autopilot_loop::TodoStatus::InProgress => accent_color,
                    crate::autopilot_loop::TodoStatus::Complete => Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0),
                    crate::autopilot_loop::TodoStatus::Failed => Hsla::new(0.0, 0.6, 0.5, 1.0),
                };
                let line = format!("{} {}", status_symbol, task.description);
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    small_font_size,
                    color,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += small_line_height;
            }
        }
        DspyStage::ExecutingTask {
            task_index,
            total_tasks,
            task_description,
        } => {
            let status = format!(
                "Task {}/{}: {}",
                task_index + 1,
                total_tasks,
                task_description
            );
            let run = cx.text.layout_styled_mono(
                &status,
                Point::new(content_x, y),
                font_size,
                accent_color,
                wgpui::text::FontStyle::default(),
            );
            cx.scene.draw_text(run);
        }
        DspyStage::TaskComplete {
            task_index,
            success,
        } => {
            let status = if *success {
                format!("Task {} completed", task_index + 1)
            } else {
                format!("Task {} failed", task_index + 1)
            };
            let color = if *success {
                Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0)
            } else {
                Hsla::new(0.0, 0.6, 0.5, 1.0)
            };
            let run = cx.text.layout_styled_mono(
                &status,
                Point::new(content_x, y),
                font_size,
                color,
                wgpui::text::FontStyle::default(),
            );
            cx.scene.draw_text(run);
        }
        DspyStage::Complete {
            total_tasks,
            successful,
            failed,
        } => {
            let summary = format!(
                "Completed {} tasks: {} successful, {} failed",
                total_tasks, successful, failed
            );
            let color = if *failed == 0 {
                Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0)
            } else {
                Hsla::new(30.0 / 360.0, 0.7, 0.5, 1.0)
            };
            let run = cx.text.layout_styled_mono(
                &summary,
                Point::new(content_x, y),
                font_size,
                color,
                wgpui::text::FontStyle::default(),
            );
            cx.scene.draw_text(run);
        }
    }
}
