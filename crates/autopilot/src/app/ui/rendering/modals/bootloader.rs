// Bootloader graph visualization renderer.
// Renders the boot sequence as a semantic graph with radial layout.

use crate::app::bootloader::{
    calculate_radial_layout, BootGraphLayout, BootNode, BootNodeType, CardState,
};

/// Render bootloader as a semantic graph in the center of the screen.
pub fn render_bootloader_center(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    logical_width: f32,
    logical_height: f32,
    _scale_factor: f32,
) {
    // Drain events and request redraw if updated
    if state.bootloader.drain_events() {
        state.window.request_redraw();
    }

    // Calculate center and radii for radial layout
    let center_x = logical_width / 2.0;
    let center_y = logical_height / 2.0;
    let inner_radius = 50.0_f32;
    let outer_radius = 150.0_f32.min(logical_width / 3.0).min(logical_height / 3.0);

    // Get the graph layout
    let mut layout = calculate_radial_layout(center_x, center_y, inner_radius, outer_radius);

    // Update node states from bootloader cards
    for card in &state.bootloader.cards {
        layout.update_stage_state(card.stage, card.state);
    }
    layout.update_primary_state();

    // Calculate card exclusion zone BEFORE drawing labels
    let card_exclusion = calculate_card_exclusion_zone(
        &state.autopilot.issue_suggestions,
        logical_width,
        logical_height,
    );

    // Draw edges first (behind nodes), skipping those in card area
    draw_edges(scene, &layout, card_exclusion.as_ref());

    // Draw nodes (skip those in card area too)
    draw_nodes(scene, &layout, card_exclusion.as_ref());

    // Draw labels (skip those in card exclusion zone)
    draw_labels(state, scene, &layout, palette, card_exclusion.as_ref());

    // Draw title at top
    draw_title(state, scene, logical_width, palette);

    // Draw error if present
    if let Some(error) = state.bootloader.error_message.clone() {
        draw_error(state, scene, &error, logical_width, logical_height, palette);
    }

    // Draw summary if boot complete
    if let Some(summary) = state.bootloader.summary.clone() {
        draw_summary(state, scene, &summary, logical_width, logical_height, palette);
    }

    // Draw issue suggestions if available
    if let Some(suggestions) = state.autopilot.issue_suggestions.clone() {
        draw_issue_suggestions(state, scene, &suggestions, logical_width, logical_height, palette);
    }
}

/// Calculate the exclusion zone for labels based on card position.
fn calculate_card_exclusion_zone(
    suggestions: &Option<DspyStage>,
    logical_width: f32,
    logical_height: f32,
) -> Option<Bounds> {
    let stage = suggestions.as_ref()?;

    match stage {
        DspyStage::IssueSuggestions { suggestions, filtered_count, await_selection, .. } => {
            if suggestions.is_empty() && *filtered_count == 0 {
                return None;
            }
            let card_width = 360.0_f32.min(logical_width - 40.0);
            let line_height = 16.0;
            let suggestion_height = suggestions.len() as f32 * line_height * 3.0;
            let filtered_height = if *filtered_count > 0 { line_height + 8.0 } else { 0.0 };
            let prompt_height = if !suggestions.is_empty() && *await_selection { line_height + 8.0 } else { 0.0 };
            let card_height = 60.0 + suggestion_height + filtered_height + prompt_height + 20.0;
            let card_x = (logical_width - card_width) / 2.0;
            let card_y = logical_height - card_height - 90.0;
            Some(Bounds::new(card_x - 10.0, card_y - 10.0, card_width + 20.0, card_height + 20.0))
        }
        DspyStage::UnblockSuggestion { .. } => {
            let card_width = 400.0_f32.min(logical_width - 40.0);
            let card_height = 210.0; // Includes prompt
            let card_x = (logical_width - card_width) / 2.0;
            let card_y = logical_height - card_height - 90.0;
            Some(Bounds::new(card_x - 10.0, card_y - 10.0, card_width + 20.0, card_height + 20.0))
        }
        _ => None,
    }
}

/// Draw bezier curve edges between nodes.
fn draw_edges(scene: &mut Scene, layout: &BootGraphLayout, exclusion_zone: Option<&Bounds>) {
    for edge in &layout.edges {
        // Skip edges whose destination (stage node) falls in exclusion zone
        // Note: We only check the 'to' point, not 'from' (primary node),
        // because the primary is at a fixed central position and checking it
        // would cause ALL edges to disappear when the card overlaps center.
        if let Some(bounds) = exclusion_zone {
            if point_in_bounds(edge.to.x, edge.to.y, Some(bounds)) {
                continue;
            }
        }
        let curve = edge.to_curve(1.5);
        scene.draw_curve(curve);
    }
}

/// Draw nodes as circles.
fn draw_nodes(scene: &mut Scene, layout: &BootGraphLayout, exclusion_zone: Option<&Bounds>) {
    // Draw origin node (hollow ring)
    if !point_in_bounds(layout.origin.position.x, layout.origin.position.y, exclusion_zone) {
        draw_circle_node(scene, &layout.origin);
    }

    // Draw primary boot node
    if !point_in_bounds(layout.primary.position.x, layout.primary.position.y, exclusion_zone) {
        draw_circle_node(scene, &layout.primary);
    }

    // Draw stage nodes
    for node in &layout.stage_nodes {
        if !point_in_bounds(node.position.x, node.position.y, exclusion_zone) {
            draw_circle_node(scene, node);
        }
    }
}

/// Draw a single node as a circle.
fn draw_circle_node(scene: &mut Scene, node: &BootNode) {
    let pos = node.position;
    let radius = node.radius;

    // Draw glow effect for running nodes
    if node.has_glow() {
        let glow_radius = radius * 2.0;
        let glow_color = Hsla::new(200.0 / 360.0, 0.5, 0.4, 0.3);
        draw_circle(scene, pos, glow_radius, glow_color, None);
    }

    // Draw the node
    let fill_color = node.fill_color();
    let border_color = node.border_color();

    // For hollow nodes (origin, pending features), draw as ring
    if matches!(node.node_type, BootNodeType::Origin)
        || (matches!(node.node_type, BootNodeType::Feature) && node.state == CardState::Pending)
    {
        // Draw border ring
        draw_circle(scene, pos, radius, border_color, None);
        // Draw transparent center (punch out)
        let inner_radius = radius - 2.0;
        if inner_radius > 0.0 {
            draw_circle(scene, pos, inner_radius, Hsla::new(0.0, 0.0, 0.04, 1.0), None);
        }
    } else {
        // Draw filled circle with border
        draw_circle(scene, pos, radius, fill_color, Some(border_color));
    }
}

/// Draw a circle using a quad with corner radius equal to half the size.
fn draw_circle(scene: &mut Scene, center: Point, radius: f32, fill: Hsla, border: Option<Hsla>) {
    let size = radius * 2.0;
    let x = center.x - radius;
    let y = center.y - radius;

    let bounds = Bounds::new(x, y, size, size);
    let mut quad = Quad::new(bounds)
        .with_background(fill)
        .with_corner_radius(radius);

    if let Some(border_color) = border {
        quad = quad.with_border(border_color, 1.0);
    }

    scene.draw_quad(quad);
}

/// Draw labels near nodes.
fn draw_labels(
    state: &mut AppState,
    scene: &mut Scene,
    layout: &BootGraphLayout,
    palette: &UiPalette,
    exclusion_zone: Option<&Bounds>,
) {
    // Label the primary boot node
    let boot_label = "Boot";
    let boot_label_width = state.text_system.measure(boot_label, 12.0);
    let boot_label_x = layout.primary.position.x - boot_label_width / 2.0;
    let boot_label_y = layout.primary.position.y + layout.primary.radius + 8.0;

    // Skip if in exclusion zone
    if !point_in_bounds(boot_label_x, boot_label_y, exclusion_zone) {
        let boot_text = state.text_system.layout_mono(
            boot_label,
            Point::new(boot_label_x, boot_label_y),
            12.0,
            palette.text_muted,
        );
        scene.draw_text(boot_text);
    }

    // Label each stage node
    for node in &layout.stage_nodes {
        if node.label.is_empty() {
            continue;
        }

        let label = &node.label;
        let label_width = state.text_system.measure(label, 10.0);

        // Position label based on node position relative to center
        let center_x = layout.origin.position.x;
        let center_y = layout.origin.position.y;

        let dx = node.position.x - center_x;
        let dy = node.position.y - center_y;

        // Label offset direction (away from center)
        let label_offset = node.radius + 12.0;
        let dist = (dx * dx + dy * dy).sqrt();
        let offset_x = if dist > 0.0 { dx / dist * label_offset } else { 0.0 };
        let offset_y = if dist > 0.0 { dy / dist * label_offset } else { label_offset };

        // Adjust for text width based on position
        let label_x = if dx > 0.0 {
            // Right side - align left edge to offset
            node.position.x + offset_x
        } else if dx < 0.0 {
            // Left side - align right edge to offset
            node.position.x + offset_x - label_width
        } else {
            // Center - center the label
            node.position.x - label_width / 2.0
        };

        let label_y = node.position.y + offset_y - 5.0; // Slight upward adjustment

        // Skip if in exclusion zone
        if point_in_bounds(label_x, label_y, exclusion_zone) {
            continue;
        }

        // Color based on state
        let label_color = match node.state {
            CardState::Complete => Hsla::new(120.0 / 360.0, 0.4, 0.5, 0.9),
            CardState::Running => Hsla::new(200.0 / 360.0, 0.5, 0.6, 1.0),
            CardState::Failed => Hsla::new(0.0, 0.5, 0.5, 1.0),
            _ => palette.text_muted,
        };

        // Add state indicator
        let state_indicator = match node.state {
            CardState::Complete => " ✓",
            CardState::Failed => " ✗",
            CardState::Running => " ...",
            _ => "",
        };
        let full_label = format!("{}{}", label, state_indicator);

        let label_text = state.text_system.layout_mono(
            &full_label,
            Point::new(label_x, label_y),
            10.0,
            label_color,
        );
        scene.draw_text(label_text);
    }
}

/// Check if a point falls within bounds (for label exclusion).
fn point_in_bounds(x: f32, y: f32, bounds: Option<&Bounds>) -> bool {
    if let Some(b) = bounds {
        x >= b.origin.x
            && x <= b.origin.x + b.size.width
            && y >= b.origin.y
            && y <= b.origin.y + b.size.height
    } else {
        false
    }
}

/// Draw title at top of screen.
fn draw_title(
    state: &mut AppState,
    scene: &mut Scene,
    logical_width: f32,
    palette: &UiPalette,
) {
    let title = "OpenAgents";
    let title_width = state.text_system.measure(title, 18.0);
    let title_x = (logical_width - title_width) / 2.0;
    let title_y = 32.0;

    let title_run = state.text_system.layout_styled_mono(
        title,
        Point::new(title_x, title_y),
        18.0,
        palette.link,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(title_run);
}

/// Draw error message.
fn draw_error(
    state: &mut AppState,
    scene: &mut Scene,
    error: &str,
    logical_width: f32,
    logical_height: f32,
    _palette: &UiPalette,
) {
    let error_text = format!("Error: {}", error);
    let error_width = state.text_system.measure(&error_text, 12.0);
    let error_x = (logical_width - error_width) / 2.0;
    let error_y = logical_height - 48.0;

    let error_run = state.text_system.layout_mono(
        &error_text,
        Point::new(error_x, error_y),
        12.0,
        Hsla::new(0.0, 0.6, 0.5, 1.0),
    );
    scene.draw_text(error_run);
}

/// Draw summary when boot complete.
fn draw_summary(
    state: &mut AppState,
    scene: &mut Scene,
    summary: &str,
    logical_width: f32,
    logical_height: f32,
    palette: &UiPalette,
) {
    let summary_y = logical_height - 64.0;

    for (i, line) in summary.lines().take(2).enumerate() {
        let line_width = state.text_system.measure(line, 11.0);
        let line_x = (logical_width - line_width) / 2.0;
        let summary_run = state.text_system.layout_mono(
            line,
            Point::new(line_x, summary_y + (i as f32 * 14.0)),
            11.0,
            palette.text_muted,
        );
        scene.draw_text(summary_run);
    }
}

/// Draw issue suggestions or unblock suggestion card.
fn draw_issue_suggestions(
    state: &mut AppState,
    scene: &mut Scene,
    stage: &DspyStage,
    logical_width: f32,
    logical_height: f32,
    palette: &UiPalette,
) {
    match stage {
        DspyStage::IssueSuggestions {
            suggestions,
            filtered_count,
            confidence,
            await_selection,
        } => {
            draw_suggestions_card(
                state, scene, suggestions, *filtered_count, *confidence, *await_selection,
                logical_width, logical_height, palette,
            );
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
            draw_unblock_card(
                state, scene, *issue_number, title, blocked_reason,
                unblock_rationale, unblock_strategy, estimated_effort, *other_blocked_count,
                logical_width, logical_height, palette,
            );
        }
        _ => {}
    }
}

/// Draw the issue suggestions card.
fn draw_suggestions_card(
    state: &mut AppState,
    scene: &mut Scene,
    suggestions: &[crate::autopilot_loop::IssueSuggestionDisplay],
    filtered_count: usize,
    confidence: f32,
    await_selection: bool,
    logical_width: f32,
    logical_height: f32,
    palette: &UiPalette,
) {
    // Show card if there are suggestions OR filtered issues to report
    if suggestions.is_empty() && filtered_count == 0 {
        return;
    }

    // Card dimensions - position just above the status bar at bottom
    let card_width = 360.0_f32.min(logical_width - 40.0);
    let line_height = 16.0;
    let suggestion_height = suggestions.len() as f32 * line_height * 3.0;
    let filtered_height = if filtered_count > 0 { line_height + 8.0 } else { 0.0 };
    let prompt_height = if !suggestions.is_empty() && await_selection { line_height + 8.0 } else { 0.0 };
    let card_height = 60.0 + suggestion_height + filtered_height + prompt_height + 20.0;
    let card_x = (logical_width - card_width) / 2.0;
    let card_y = logical_height - card_height - 90.0;

    // Card background
    scene.draw_quad(
        Quad::new(Bounds::new(card_x, card_y, card_width, card_height))
            .with_background(palette.panel)
            .with_border(Hsla::new(45.0 / 360.0, 0.7, 0.5, 1.0), 2.0) // Gold
            .with_corner_radius(8.0),
    );

    let mut y = card_y + 12.0;
    let font_size = 12.0;
    let small_font_size = 11.0;

    // Header
    let header = if suggestions.is_empty() {
        "Issue Suggestions"
    } else {
        "Suggested Issues"
    };
    let header_run = state.text_system.layout_mono(
        header,
        Point::new(card_x + 12.0, y),
        font_size,
        Hsla::new(45.0 / 360.0, 0.7, 0.5, 1.0),
    );
    scene.draw_text(header_run);
    y += line_height + 4.0;

    // Status line
    let status_line = if suggestions.is_empty() {
        "All issues blocked or stale".to_string()
    } else {
        let status = if await_selection { "Select an issue:" } else { "Auto-selecting..." };
        format!("Confidence: {:.0}% · {}", confidence * 100.0, status)
    };
    let status_run = state.text_system.layout_mono(
        &status_line,
        Point::new(card_x + 12.0, y),
        small_font_size,
        palette.text_muted,
    );
    scene.draw_text(status_run);
    y += line_height + 8.0;

    // Suggestions
    for (i, suggestion) in suggestions.iter().enumerate() {
        let title_line = format!(
            "{}. [#{}] {} ({})",
            i + 1,
            suggestion.number,
            truncate(&suggestion.title, 35),
            suggestion.priority
        );
        let title_run = state.text_system.layout_mono(
            &title_line,
            Point::new(card_x + 12.0, y),
            small_font_size,
            palette.text_primary,
        );
        scene.draw_text(title_run);
        y += line_height;

        let rationale_line = format!("   \"{}\"", truncate(&suggestion.rationale, 40));
        let rationale_run = state.text_system.layout_mono(
            &rationale_line,
            Point::new(card_x + 12.0, y),
            small_font_size,
            palette.text_muted,
        );
        scene.draw_text(rationale_run);
        y += line_height;

        let complexity_line = format!("   Complexity: {}", suggestion.complexity);
        let complexity_run = state.text_system.layout_mono(
            &complexity_line,
            Point::new(card_x + 12.0, y),
            small_font_size,
            palette.text_dim,
        );
        scene.draw_text(complexity_run);
        y += line_height + 4.0;
    }

    // Filtered count
    if filtered_count > 0 {
        let filtered_line = format!("[{} issues filtered as stale/blocked]", filtered_count);
        let filtered_run = state.text_system.layout_mono(
            &filtered_line,
            Point::new(card_x + 12.0, y),
            small_font_size,
            palette.text_dim,
        );
        scene.draw_text(filtered_run);
        y += line_height;
    }

    // Selection prompt (only if there are suggestions to select)
    if !suggestions.is_empty() && await_selection {
        y += 8.0;
        let prompt = format!("Select 1-{}, or [S]kip:", suggestions.len());
        let prompt_run = state.text_system.layout_mono(
            &prompt,
            Point::new(card_x + 12.0, y),
            small_font_size,
            Hsla::new(45.0 / 360.0, 0.7, 0.5, 1.0), // Gold accent
        );
        scene.draw_text(prompt_run);
    }
}

/// Draw the unblock suggestion card.
#[allow(clippy::too_many_arguments)]
fn draw_unblock_card(
    state: &mut AppState,
    scene: &mut Scene,
    issue_number: u32,
    title: &str,
    blocked_reason: &str,
    unblock_rationale: &str,
    unblock_strategy: &str,
    estimated_effort: &str,
    other_blocked_count: usize,
    logical_width: f32,
    logical_height: f32,
    palette: &UiPalette,
) {
    let card_width = 400.0_f32.min(logical_width - 40.0);
    let line_height = 16.0;
    let card_height = 210.0; // Fixed height for unblock card (includes prompt)
    let card_x = (logical_width - card_width) / 2.0;
    let card_y = logical_height - card_height - 90.0;

    // Card background with teal border (unblock color)
    let accent_color = Hsla::new(160.0 / 360.0, 0.6, 0.5, 1.0);
    scene.draw_quad(
        Quad::new(Bounds::new(card_x, card_y, card_width, card_height))
            .with_background(palette.panel)
            .with_border(accent_color, 2.0)
            .with_corner_radius(8.0),
    );

    let mut y = card_y + 12.0;
    let font_size = 12.0;
    let small_font_size = 11.0;

    // Header
    let header_run = state.text_system.layout_mono(
        "Suggested Issue to Unblock",
        Point::new(card_x + 12.0, y),
        font_size,
        accent_color,
    );
    scene.draw_text(header_run);
    y += line_height + 8.0;

    // Calculate max chars based on card width (mono font ~6.6px per char at 11px)
    let content_width = card_width - 24.0; // 12px padding each side
    let char_width = small_font_size * 0.6;
    let max_chars = (content_width / char_width) as usize;

    // Issue title
    let title_line = format!("#{} {}", issue_number, truncate(title, max_chars.saturating_sub(6)));
    let title_run = state.text_system.layout_mono(
        &title_line,
        Point::new(card_x + 12.0, y),
        font_size,
        palette.text_primary,
    );
    scene.draw_text(title_run);
    y += line_height + 4.0;

    // Blocked reason
    let blocked_line = format!("Blocked: \"{}\"", truncate(blocked_reason, max_chars.saturating_sub(14)));
    let blocked_run = state.text_system.layout_mono(
        &blocked_line,
        Point::new(card_x + 12.0, y),
        small_font_size,
        Hsla::new(0.0, 0.5, 0.6, 1.0), // Red-ish for blocked
    );
    scene.draw_text(blocked_run);
    y += line_height + 8.0;

    // Why unblock this first
    let why_line = format!("Why: {}", truncate(unblock_rationale, max_chars.saturating_sub(8)));
    let why_run = state.text_system.layout_mono(
        &why_line,
        Point::new(card_x + 12.0, y),
        small_font_size,
        palette.text_muted,
    );
    scene.draw_text(why_run);
    y += line_height + 4.0;

    // Strategy
    let strategy_line = format!("Strategy: {}", truncate(unblock_strategy, max_chars.saturating_sub(13)));
    let strategy_run = state.text_system.layout_mono(
        &strategy_line,
        Point::new(card_x + 12.0, y),
        small_font_size,
        palette.text_muted,
    );
    scene.draw_text(strategy_run);
    y += line_height + 4.0;

    // Effort
    let effort_line = format!("Effort: {}", estimated_effort);
    let effort_run = state.text_system.layout_mono(
        &effort_line,
        Point::new(card_x + 12.0, y),
        small_font_size,
        palette.text_dim,
    );
    scene.draw_text(effort_run);
    y += line_height + 8.0;

    // Other blocked count
    if other_blocked_count > 0 {
        let other_line = format!("[{} other issues also blocked]", other_blocked_count);
        let other_run = state.text_system.layout_mono(
            &other_line,
            Point::new(card_x + 12.0, y),
            small_font_size,
            palette.text_dim,
        );
        scene.draw_text(other_run);
        y += line_height;
    }

    // Selection prompt
    y += 8.0;
    let prompt = "Work on this issue? [Y/n]";
    let prompt_run = state.text_system.layout_mono(
        prompt,
        Point::new(card_x + 12.0, y),
        small_font_size,
        accent_color,
    );
    scene.draw_text(prompt_run);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
