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
    let content_width = (bounds.size.width - padding * 2.0).max(0.0);
    let wrap_chars_small =
        ((content_width / (small_font_size * 0.6)).floor().max(16.0)) as usize;
    let wrap_chars_large = ((content_width / (font_size * 0.6)).floor().max(12.0)) as usize;

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
        DspyStage::IssueSuggestions { .. } => (
            "Issue Suggestions",
            Hsla::new(45.0 / 360.0, 0.7, 0.5, 1.0), // Gold
            "📋",
        ),
        DspyStage::IssueSelected { .. } => (
            "Issue Selected",
            Hsla::new(160.0 / 360.0, 0.6, 0.5, 1.0), // Teal
            "🎯",
        ),
        DspyStage::UnblockSuggestion { .. } => (
            "Unblock Suggestion",
            Hsla::new(160.0 / 360.0, 0.6, 0.5, 1.0), // Teal
            "🔓",
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
            priority_action,
            urgency,
            reasoning,
            ..
        } => {
            let directive = active_directive.as_deref().unwrap_or("None");
            let priority_line = format!("{} ({})", priority_action, urgency);
            let reasoning_line = truncate_preview(reasoning, 140);
            let mut items = vec![
                ("System".to_string(), truncate_preview(system_info, 120)),
                ("Workspace".to_string(), truncate_preview(workspace, 120)),
                ("Directive".to_string(), truncate_preview(directive, 120)),
                ("Open issues".to_string(), open_issues.to_string()),
                ("Priority".to_string(), priority_line),
            ];
            if !reasoning_line.is_empty() {
                items.push(("Reasoning".to_string(), reasoning_line));
            }
            for (label, text) in items {
                let line = format!("{}: {}", label, text);
                for wrapped in wrap_text(&line, wrap_chars_small) {
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
            let clean_analysis = strip_markdown_markers(&truncate_preview(analysis, 160));
            let analysis_line = format!("Analysis: {}", clean_analysis);
            for line in wrap_text(&analysis_line, wrap_chars_small) {
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
                format!("Complexity: {} · Confidence: {:.0}%", complexity, confidence * 100.0);
            let run = cx.text.layout_styled_mono(
                &complexity_line,
                Point::new(content_x, y),
                small_font_size,
                palette.text_dim,
                wgpui::text::FontStyle::default(),
            );
            cx.scene.draw_text(run);
            y += small_line_height + 4.0;

            let clean_test = strip_markdown_markers(&truncate_preview(test_strategy, 160));
            let test_line = format!("Test: {}", clean_test);
            for line in wrap_text(&test_line, wrap_chars_small) {
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

            for step in implementation_steps.iter() {
                // Steps are already numbered from LLM (e.g., "1. **Examine Current State**")
                // Strip markdown markers and truncate
                let clean_step = strip_markdown_markers(&truncate_preview(step, 80));
                for line in wrap_text(&clean_step, wrap_chars_small) {
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
                // Strip markdown from todo description
                let clean_desc = strip_markdown_markers(&truncate_preview(&task.description, 80));
                let line = format!("{} {}", status_symbol, clean_desc);
                for wrapped in wrap_text(&line, wrap_chars_small) {
                    let run = cx.text.layout_styled_mono(
                        &wrapped,
                        Point::new(content_x, y),
                        small_font_size,
                        color,
                        wgpui::text::FontStyle::default(),
                    );
                    cx.scene.draw_text(run);
                    y += small_line_height;
                }
            }
        }
        DspyStage::ExecutingTask {
            task_index,
            total_tasks,
            task_description,
        } => {
            // task_index is already 1-indexed from autopilot_loop
            // Strip markdown from task description
            let clean_desc = strip_markdown_markers(&truncate_preview(task_description, 60));
            let status = format!("Task {}/{}: {}", task_index, total_tasks, clean_desc);
            for line in wrap_text(&status, wrap_chars_large) {
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    font_size,
                    accent_color,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += line_height;
            }
        }
        DspyStage::TaskComplete {
            task_index,
            success,
        } => {
            // task_index is already 1-indexed
            let status = if *success {
                format!("Task {} completed", task_index)
            } else {
                format!("Task {} failed", task_index)
            };
            let color = if *success {
                Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0)
            } else {
                Hsla::new(0.0, 0.6, 0.5, 1.0)
            };
            for line in wrap_text(&status, wrap_chars_large) {
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    font_size,
                    color,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += line_height;
            }
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
            for line in wrap_text(&summary, wrap_chars_large) {
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    font_size,
                    color,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += line_height;
            }
        }
        DspyStage::IssueSuggestions {
            suggestions,
            filtered_count,
            confidence,
            await_selection,
        } => {
            // Show confidence and selection status
            let status = if *await_selection { "Awaiting selection..." } else { "Auto-selecting..." };
            let status_line = format!("Confidence: {:.0}% · {}", confidence * 100.0, status);
            for line in wrap_text(&status_line, wrap_chars_small) {
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
            y += 4.0;

            // Show each suggestion
            for (i, suggestion) in suggestions.iter().enumerate() {
                let title_line = format!(
                    "{}. [#{}] {} ({})",
                    i + 1,
                    suggestion.number,
                    truncate_preview(&suggestion.title, 50),
                    suggestion.priority
                );
                for line in wrap_text(&title_line, wrap_chars_small) {
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

                let rationale_line = format!("   \"{}\"", truncate_preview(&suggestion.rationale, 60));
                for line in wrap_text(&rationale_line, wrap_chars_small) {
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

                let complexity_line = format!("   Complexity: {}", suggestion.complexity);
                for line in wrap_text(&complexity_line, wrap_chars_small) {
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
                y += 4.0;
            }

            // Show filtered count
            if *filtered_count > 0 {
                let filtered_line = format!("[{} issues filtered as stale/blocked]", filtered_count);
                for line in wrap_text(&filtered_line, wrap_chars_small) {
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
            }
        }
        DspyStage::IssueSelected {
            number,
            title,
            selection_method,
        } => {
            let summary = format!(
                "Selected issue #{}: {} ({})",
                number,
                truncate_preview(title, 50),
                selection_method
            );
            for line in wrap_text(&summary, wrap_chars_large) {
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    font_size,
                    accent_color,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += line_height;
            }
        }
        DspyStage::UnblockSuggestion {
            issue_number,
            title,
            blocked_reason,
            unblock_rationale,
            unblock_strategy,
            estimated_effort,
            other_blocked_count,
        } => {
            // Issue title
            let title_line = format!("#{} {}", issue_number, truncate_preview(title, 50));
            for line in wrap_text(&title_line, wrap_chars_large) {
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    font_size,
                    palette.text_primary,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += line_height;
            }
            y += 4.0;

            // Blocked reason
            let blocked_line = format!("Blocked: \"{}\"", truncate_preview(blocked_reason, 60));
            for line in wrap_text(&blocked_line, wrap_chars_small) {
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    small_font_size,
                    Hsla::new(0.0, 0.5, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += small_line_height;
            }
            y += 4.0;

            // Rationale
            let why_line = format!("Why: {}", truncate_preview(unblock_rationale, 60));
            for line in wrap_text(&why_line, wrap_chars_small) {
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    small_font_size,
                    palette.text_muted,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += small_line_height;
            }

            // Strategy
            let strategy_line = format!("Strategy: {}", truncate_preview(unblock_strategy, 55));
            for line in wrap_text(&strategy_line, wrap_chars_small) {
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    small_font_size,
                    palette.text_muted,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += small_line_height;
            }

            // Effort + other count
            let effort_line = format!("Effort: {} | {} other blocked", estimated_effort, other_blocked_count);
            for line in wrap_text(&effort_line, wrap_chars_small) {
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
        }
    }
}
