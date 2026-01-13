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

    // Draw edges first (behind nodes)
    draw_edges(scene, &layout);

    // Draw nodes
    draw_nodes(scene, &layout);

    // Draw labels
    draw_labels(state, scene, &layout, palette);

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
}

/// Draw bezier curve edges between nodes.
fn draw_edges(scene: &mut Scene, layout: &BootGraphLayout) {
    for edge in &layout.edges {
        let curve = edge.to_curve(1.5);
        scene.draw_curve(curve);
    }
}

/// Draw nodes as circles.
fn draw_nodes(scene: &mut Scene, layout: &BootGraphLayout) {
    // Draw origin node (hollow ring)
    draw_circle_node(scene, &layout.origin);

    // Draw primary boot node
    draw_circle_node(scene, &layout.primary);

    // Draw stage nodes
    for node in &layout.stage_nodes {
        draw_circle_node(scene, node);
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
) {
    // Label the primary boot node
    let boot_label = "Boot";
    let boot_label_width = state.text_system.measure(boot_label, 12.0);
    let boot_label_x = layout.primary.position.x - boot_label_width / 2.0;
    let boot_label_y = layout.primary.position.y + layout.primary.radius + 8.0;

    let boot_text = state.text_system.layout_mono(
        boot_label,
        Point::new(boot_label_x, boot_label_y),
        12.0,
        palette.text_muted,
    );
    scene.draw_text(boot_text);

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
