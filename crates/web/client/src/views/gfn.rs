//! GFN (Group Forming Networks) page view
//!
//! Interactive visualization demonstrating the difference between
//! Metcalfe's Law (N²) and Reed's Law (2^N).

use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem, theme};
use wgpui::animation::AnimatorState;
use wgpui::components::Component;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::PaintContext;

use crate::state::AppState;

/// Light blue color for Metcalfe's Law
const METCALFE_COLOR: Hsla = Hsla::new(0.556, 0.7, 0.6, 1.0);

/// Orange (#FF9900) color for Reed's Law
const REED_COLOR: Hsla = Hsla::new(0.1, 1.0, 0.5, 1.0);

/// Build the GFN page view
pub(crate) fn build_gfn_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    // Background
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP),
    );

    // Dots grid background - brighter white
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 1.0, 0.35))
        .distance(36.0)
        .size(1.0)
        .origin(DotsOrigin::Center);
    state.dots_grid.update(AnimatorState::Entered);
    state.dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // Layout calculations
    let padding = 24.0;
    let content_width = (width - padding * 2.0).min(900.0);
    let content_x = (width - content_width) / 2.0;

    // Start frame animation
    if !state.gfn.frame_started {
        state.gfn.frame_started = true;
    }
    let frame_progress = state.gfn.frame_animator.update(AnimatorState::Entering);

    // Main card dimensions - responsive
    let card_y = padding;
    let card_height = height - padding * 2.0;
    let card_bounds = Bounds::new(content_x, card_y, content_width, card_height);

    // Draw main frame with corners
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    let mut frame = Frame::corners()
        .line_color(Hsla::new(0.0, 0.0, 1.0, 0.8))
        .bg_color(Hsla::new(0.0, 0.0, 0.0, 0.4))
        .glow_color(Hsla::new(0.0, 0.0, 1.0, 0.15))
        .border_color(Hsla::new(0.0, 0.0, 1.0, 0.5))
        .stroke_width(1.0)
        .corner_length(30.0)
        .animation_progress(frame_progress);
    frame.paint(card_bounds, &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // Inner content padding
    let inner_padding = 32.0;
    let scrollbar_width = 12.0;
    let inner_x = content_x + inner_padding;
    let inner_width = content_width - inner_padding * 2.0 - scrollbar_width;
    let visible_height = card_height - inner_padding * 2.0;

    // Store content bounds for scroll detection
    state.gfn.content_bounds = Bounds::new(inner_x, card_y + inner_padding, inner_width + scrollbar_width, visible_height);

    // Push clip for scrollable content
    scene.push_clip(Bounds::new(inner_x - 4.0, card_y + inner_padding, inner_width + scrollbar_width + 8.0, visible_height));

    // Apply scroll offset
    let scroll_offset = state.gfn.scroll_offset;
    let mut y = card_y + inner_padding - scroll_offset;

    // Title
    let title = "GROUP FORMING NETWORKS";
    let title_size = 24.0;
    let title_width = text_system.measure(title, title_size);
    let title_x = inner_x + (inner_width - title_width) / 2.0;
    let title_run = text_system.layout(title, Point::new(title_x, y), title_size, theme::text::PRIMARY);
    scene.draw_text(title_run);
    y += title_size + 8.0;

    // Subtitle
    let subtitle = "Metcalfe vs Reed's Law";
    let subtitle_size = 14.0;
    let subtitle_width = text_system.measure(subtitle, subtitle_size);
    let subtitle_x = inner_x + (inner_width - subtitle_width) / 2.0;
    let subtitle_run = text_system.layout(subtitle, Point::new(subtitle_x, y), subtitle_size, theme::text::MUTED);
    scene.draw_text(subtitle_run);
    y += subtitle_size + 24.0;

    // Network diagrams section
    let n = state.gfn.node_count;
    let diagram_size = ((inner_width - 40.0) / 2.0).min(200.0);
    let diagram_y = y;

    // Metcalfe network (left)
    let metcalfe_x = inner_x + (inner_width / 2.0 - diagram_size) / 2.0;
    let metcalfe_bounds = Bounds::new(metcalfe_x, diagram_y, diagram_size, diagram_size);
    draw_metcalfe_network(scene, metcalfe_bounds, n);

    // Reed network (right)
    let reed_x = inner_x + inner_width / 2.0 + (inner_width / 2.0 - diagram_size) / 2.0;
    let reed_bounds = Bounds::new(reed_x, diagram_y, diagram_size, diagram_size);
    draw_reed_network(scene, reed_bounds, n, text_system);

    // Labels under diagrams
    let label_y = diagram_y + diagram_size + 8.0;

    // Metcalfe label
    let metcalfe_value = n * n; // N squared
    let metcalfe_label = format!("N^2 = {}", metcalfe_value);
    let metcalfe_label_width = text_system.measure(&metcalfe_label, 14.0);
    let metcalfe_label_x = metcalfe_x + (diagram_size - metcalfe_label_width) / 2.0;
    let metcalfe_run = text_system.layout(&metcalfe_label, Point::new(metcalfe_label_x, label_y), 14.0, METCALFE_COLOR);
    scene.draw_text(metcalfe_run);

    let metcalfe_title = "METCALFE";
    let metcalfe_title_width = text_system.measure(metcalfe_title, 11.0);
    let metcalfe_title_x = metcalfe_x + (diagram_size - metcalfe_title_width) / 2.0;
    let metcalfe_title_run = text_system.layout(metcalfe_title, Point::new(metcalfe_title_x, label_y + 18.0), 11.0, theme::text::MUTED);
    scene.draw_text(metcalfe_title_run);

    // Reed label - correct formula is 2^N - N - 1 (non-trivial subsets)
    let reed_label = if n <= 20 {
        let reed_value = (1u64 << n).saturating_sub(n as u64).saturating_sub(1);
        format!("2^N-N-1 = {}", reed_value)
    } else if n <= 30 {
        // Show in millions/billions for readability
        let reed_value = (1u64 << n).saturating_sub(n as u64).saturating_sub(1);
        if reed_value >= 1_000_000_000 {
            format!("2^N-N-1 = {:.1}B", reed_value as f64 / 1_000_000_000.0)
        } else {
            format!("2^N-N-1 = {:.1}M", reed_value as f64 / 1_000_000.0)
        }
    } else {
        // For very large N, show as power of 2
        format!("2^N-N-1 ~ 2^{}", n)
    };
    let reed_label_width = text_system.measure(&reed_label, 14.0);
    let reed_label_x = reed_x + (diagram_size - reed_label_width) / 2.0;
    let reed_run = text_system.layout(&reed_label, Point::new(reed_label_x, label_y), 14.0, REED_COLOR);
    scene.draw_text(reed_run);

    let reed_title = "REED";
    let reed_title_width = text_system.measure(reed_title, 11.0);
    let reed_title_x = reed_x + (diagram_size - reed_title_width) / 2.0;
    let reed_title_run = text_system.layout(reed_title, Point::new(reed_title_x, label_y + 18.0), 11.0, theme::text::MUTED);
    scene.draw_text(reed_title_run);

    y = label_y + 44.0;

    // Slider section
    let slider_label = format!("Nodes: {}", n);
    let slider_label_width = text_system.measure(&slider_label, 14.0);
    let slider_label_run = text_system.layout(&slider_label, Point::new(inner_x, y), 14.0, theme::text::PRIMARY);
    scene.draw_text(slider_label_run);

    let slider_x = inner_x + slider_label_width + 16.0;
    let slider_width = inner_width - slider_label_width - 16.0;
    let slider_bounds = Bounds::new(slider_x, y - 4.0, slider_width, 24.0);
    state.gfn.slider_bounds = slider_bounds;
    draw_slider(scene, slider_bounds, n, 2, 50);
    y += 32.0;

    // Comparison graph - fixed height, don't compress on scroll
    let graph_height = 300.0;
    let graph_bounds = Bounds::new(inner_x, y, inner_width, graph_height);
    draw_comparison_graph(scene, text_system, graph_bounds, n);
    y += graph_height + 24.0;

    // Two-column explanatory text - ROW 1: The Laws vs 50 Agents
    let col_width = (inner_width - 24.0) / 2.0;
    let col1_x = inner_x;
    let col2_x = inner_x + col_width + 24.0;
    let line_height = 14.0;
    let font_size = 9.0;
    let header_size = 10.0;

    // Row 1 Left: Law definitions
    let row1_left = [
        ("THE LAWS", theme::text::PRIMARY, true),
        ("METCALFE: VALUE = N^2", METCALFE_COLOR, true),
        ("Pairwise connections. Phones,", theme::text::MUTED, false),
        ("fax, social networks.", theme::text::MUTED, false),
        ("REED: VALUE = 2^N - N - 1", REED_COLOR, true),
        ("Subsets multiply. Each new", theme::text::MUTED, false),
        ("member doubles combinations.", theme::text::MUTED, false),
    ];

    // Row 1 Right: 50 Agents example
    let row1_right = [
        ("50 AGENTS (SLIDER MAX)", theme::text::PRIMARY, true),
        ("N^2 = 2,500 CONNECTIONS", METCALFE_COLOR, true),
        ("Basic task routing. Agent A", theme::text::MUTED, false),
        ("asks Agent B for help.", theme::text::MUTED, false),
        ("2^50 = 10^15 COALITIONS", REED_COLOR, true),
        ("Specialized teams form for", theme::text::MUTED, false),
        ("complex multi-step tasks.", theme::text::MUTED, false),
    ];

    let mut line_y = y;
    for (i, (line, color, is_header)) in row1_left.iter().enumerate() {
        if !line.is_empty() {
            let size = if *is_header { header_size } else { font_size };
            let run = text_system.layout(line, Point::new(col1_x, line_y), size, *color);
            scene.draw_text(run);
        }
        if i < row1_right.len() {
            let (rline, rcolor, ris_header) = &row1_right[i];
            if !rline.is_empty() {
                let size = if *ris_header { header_size } else { font_size };
                let run = text_system.layout(rline, Point::new(col2_x, line_y), size, *rcolor);
                scene.draw_text(run);
            }
        }
        line_y += line_height;
    }

    // Divider
    line_y += 6.0;
    scene.draw_quad(
        Quad::new(Bounds::new(inner_x, line_y, inner_width, 1.0))
            .with_background(theme::border::DEFAULT.with_alpha(0.4)),
    );
    line_y += 10.0;

    // Row 2: What You Get
    let row2_left = [
        ("WHAT YOU GET", REED_COLOR, true),
        ("Expert help in any field for", theme::text::MUTED, false),
        ("pennies. Medical advice from", theme::text::MUTED, false),
        ("1000 specialists. Legal counsel", theme::text::MUTED, false),
        ("for $1. Tax prep. Tutoring.", theme::text::MUTED, false),
    ];
    let row2_right = [
        ("YOUR PROJECTS", REED_COLOR, true),
        ("Launch a business with a team", theme::text::MUTED, false),
        ("of 50 agents. Write a book.", theme::text::MUTED, false),
        ("Make a game. Build an app.", theme::text::MUTED, false),
        ("Things that took teams: now you.", theme::text::MUTED, false),
    ];

    for (i, (line, color, is_header)) in row2_left.iter().enumerate() {
        if !line.is_empty() {
            let size = if *is_header { header_size } else { font_size };
            let run = text_system.layout(line, Point::new(col1_x, line_y), size, *color);
            scene.draw_text(run);
        }
        if i < row2_right.len() {
            let (rline, rcolor, ris_header) = &row2_right[i];
            if !rline.is_empty() {
                let size = if *ris_header { header_size } else { font_size };
                let run = text_system.layout(rline, Point::new(col2_x, line_y), size, *rcolor);
                scene.draw_text(run);
            }
        }
        line_y += line_height;
    }
    line_y += 4.0;

    // Row 3: Learning & Opportunity
    let row3_left = [
        ("YOUR EDUCATION", REED_COLOR, true),
        ("Personalized tutoring in any", theme::text::MUTED, false),
        ("subject. Learn at your pace.", theme::text::MUTED, false),
        ("Master new skills 10x faster.", theme::text::MUTED, false),
        ("World-class teaching for all.", theme::text::MUTED, false),
    ];
    let row3_right = [
        ("YOUR OPPORTUNITY", REED_COLOR, true),
        ("Compete with corporations.", theme::text::MUTED, false),
        ("Your small business gets the", theme::text::MUTED, false),
        ("same capabilities as Fortune", theme::text::MUTED, false),
        ("500. Level playing field.", theme::text::MUTED, false),
    ];

    for (i, (line, color, is_header)) in row3_left.iter().enumerate() {
        if !line.is_empty() {
            let size = if *is_header { header_size } else { font_size };
            let run = text_system.layout(line, Point::new(col1_x, line_y), size, *color);
            scene.draw_text(run);
        }
        if i < row3_right.len() {
            let (rline, rcolor, ris_header) = &row3_right[i];
            if !rline.is_empty() {
                let size = if *ris_header { header_size } else { font_size };
                let run = text_system.layout(rline, Point::new(col2_x, line_y), size, *rcolor);
                scene.draw_text(run);
            }
        }
        line_y += line_height;
    }
    line_y += 4.0;

    // Row 4: Time & Money
    let row4_left = [
        ("YOUR TIME", REED_COLOR, true),
        ("Delegate the tedious. Focus on", theme::text::MUTED, false),
        ("what matters to you. Spend time", theme::text::MUTED, false),
        ("with family. Pursue passions.", theme::text::MUTED, false),
        ("Agents handle the rest.", theme::text::MUTED, false),
    ];
    let row4_right = [
        ("YOUR MONEY", REED_COLOR, true),
        ("Services that cost $1000s now", theme::text::MUTED, false),
        ("cost dollars. Expertise without", theme::text::MUTED, false),
        ("gatekeepers. No middlemen.", theme::text::MUTED, false),
        ("Direct value exchange.", theme::text::MUTED, false),
    ];

    for (i, (line, color, is_header)) in row4_left.iter().enumerate() {
        if !line.is_empty() {
            let size = if *is_header { header_size } else { font_size };
            let run = text_system.layout(line, Point::new(col1_x, line_y), size, *color);
            scene.draw_text(run);
        }
        if i < row4_right.len() {
            let (rline, rcolor, ris_header) = &row4_right[i];
            if !rline.is_empty() {
                let size = if *ris_header { header_size } else { font_size };
                let run = text_system.layout(rline, Point::new(col2_x, line_y), size, *rcolor);
                scene.draw_text(run);
            }
        }
        line_y += line_height;
    }
    line_y += 8.0;

    // The key insight
    let meta_header = "THE KEY INSIGHT";
    let meta_run = text_system.layout(meta_header, Point::new(col1_x, line_y), header_size, REED_COLOR);
    scene.draw_text(meta_run);
    line_y += line_height + 2.0;

    let meta_lines = [
        "You become a coordinator of expert teams, not a lone worker.",
        "Your ideas + agent coalitions = capabilities that used to require",
        "entire companies. The playing field has never been more level.",
    ];
    for line in &meta_lines {
        let run = text_system.layout(line, Point::new(col1_x, line_y), font_size, theme::text::MUTED);
        scene.draw_text(run);
        line_y += line_height;
    }

    y = line_y + 8.0;

    // CTA button
    let cta_text = "Read More";
    let cta_font_size = 14.0;
    let cta_padding_h = 24.0;
    let cta_padding_v = 10.0;
    let cta_width = text_system.measure(cta_text, cta_font_size) + cta_padding_h * 2.0;
    let cta_height = cta_font_size + cta_padding_v * 2.0;
    let cta_x = inner_x + (inner_width - cta_width) / 2.0;
    let cta_y = y;

    state.gfn.cta_bounds = Bounds::new(cta_x, cta_y, cta_width, cta_height);

    let cta_bg = if state.gfn.cta_hovered {
        theme::bg::ELEVATED
    } else {
        theme::bg::SURFACE
    };
    let cta_border = if state.gfn.cta_hovered {
        METCALFE_COLOR
    } else {
        theme::border::DEFAULT
    };

    scene.draw_quad(
        Quad::new(state.gfn.cta_bounds)
            .with_background(cta_bg)
            .with_border(cta_border, 1.0)
            .with_corner_radius(2.0),
    );

    let cta_text_x = cta_x + cta_padding_h;
    let cta_text_y = cta_y + cta_padding_v;
    let cta_run = text_system.layout(cta_text, Point::new(cta_text_x, cta_text_y), cta_font_size, theme::text::PRIMARY);
    scene.draw_text(cta_run);

    // End of content - calculate total height
    y += cta_height + 24.0;
    let total_content_height = y - (card_y + inner_padding - scroll_offset);
    state.gfn.content_height = total_content_height;

    // Pop the clip
    scene.pop_clip();

    // Draw scrollbar if content overflows
    let max_scroll = (total_content_height - visible_height).max(0.0);
    state.gfn.scroll_offset = state.gfn.scroll_offset.clamp(0.0, max_scroll);

    if max_scroll > 0.0 {
        // Position at right edge of frame, full height
        let scrollbar_x = content_x + content_width - scrollbar_width - 4.0;
        let scrollbar_y = card_y + 8.0;
        let scrollbar_height = card_height - 16.0;

        // HUD-style track: dark interior with glowing white outline
        scene.draw_quad(
            Quad::new(Bounds::new(scrollbar_x, scrollbar_y, scrollbar_width, scrollbar_height))
                .with_background(Hsla::new(0.0, 0.0, 0.02, 0.9))
                .with_border(Hsla::new(0.0, 0.0, 1.0, 0.4), 1.0)
                .with_corner_radius(2.0),
        );
        // Outer glow layer
        scene.draw_quad(
            Quad::new(Bounds::new(scrollbar_x - 1.0, scrollbar_y - 1.0, scrollbar_width + 2.0, scrollbar_height + 2.0))
                .with_background(Hsla::new(0.0, 0.0, 0.0, 0.0))
                .with_border(Hsla::new(0.0, 0.0, 1.0, 0.15), 1.0)
                .with_corner_radius(3.0),
        );

        // HUD-style thumb: bright outline, dark fill
        let thumb_height = (visible_height / total_content_height * scrollbar_height).max(40.0);
        let thumb_progress = state.gfn.scroll_offset / max_scroll;
        let thumb_y = scrollbar_y + 2.0 + thumb_progress * (scrollbar_height - thumb_height - 4.0);

        // Thumb inner glow
        scene.draw_quad(
            Quad::new(Bounds::new(scrollbar_x + 2.0, thumb_y, scrollbar_width - 4.0, thumb_height))
                .with_background(Hsla::new(0.0, 0.0, 0.1, 0.95))
                .with_border(Hsla::new(0.0, 0.0, 1.0, 0.8), 1.0)
                .with_corner_radius(2.0),
        );
        // Thumb center highlight
        scene.draw_quad(
            Quad::new(Bounds::new(scrollbar_x + 4.0, thumb_y + thumb_height / 2.0 - 4.0, scrollbar_width - 8.0, 8.0))
                .with_background(Hsla::new(0.0, 0.0, 1.0, 0.3))
                .with_corner_radius(1.0),
        );
    }

    // Clear other page state
    state.button_bounds = Bounds::ZERO;
    state.left_cta_bounds = Bounds::ZERO;
    state.right_cta_bounds = Bounds::ZERO;
    state.landing_issue_bounds = Bounds::ZERO;
    state.repo_bounds.clear();
}

/// Draw the Metcalfe network diagram (pairwise connections)
fn draw_metcalfe_network(scene: &mut Scene, bounds: Bounds, n: u32) {
    let center = Point::new(bounds.x() + bounds.width() / 2.0, bounds.y() + bounds.height() / 2.0);
    let radius = (bounds.width().min(bounds.height()) / 2.0) - 12.0;

    // Calculate node positions in a circle
    let nodes: Vec<Point> = (0..n).map(|i| {
        let angle = (i as f32 / n as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        Point::new(
            center.x + angle.cos() * radius,
            center.y + angle.sin() * radius,
        )
    }).collect();

    // Draw connections (all pairs) - blue with low alpha
    let line_color = METCALFE_COLOR.with_alpha(0.25);
    for i in 0..n as usize {
        for j in (i + 1)..n as usize {
            draw_line(scene, nodes[i], nodes[j], line_color);
        }
    }

    // Draw nodes - blue
    let node_color = METCALFE_COLOR;
    let node_radius = 4.0;
    for node in &nodes {
        draw_circle(scene, *node, node_radius, node_color);
    }
}

/// Draw the Reed network diagram (coalition groupings)
/// Shows nodes around perimeter with dots representing groups in center
/// Draws connections from nodes to group dots
fn draw_reed_network(scene: &mut Scene, bounds: Bounds, n: u32, _text_system: &mut TextSystem) {
    let center = Point::new(bounds.x() + bounds.width() / 2.0, bounds.y() + bounds.height() / 2.0);
    let radius = (bounds.width().min(bounds.height()) / 2.0) - 12.0;

    // Calculate node positions in a circle (same as Metcalfe)
    let nodes: Vec<Point> = (0..n).map(|i| {
        let angle = (i as f32 / n as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        Point::new(
            center.x + angle.cos() * radius,
            center.y + angle.sin() * radius,
        )
    }).collect();

    let inner_radius = radius * 0.65;

    // Calculate dots to show using correct Reed's Law: 2^N - N - 1
    let base_dots = if n <= 1 {
        1.0
    } else if n <= 20 {
        let groups = (1u64 << n).saturating_sub(n as u64).saturating_sub(1);
        groups as f64
    } else {
        let base = (1u64 << 20) - 20 - 1;
        (base as f64) * (1.0 + ((n - 20) as f64 * 0.15))
    };

    let max_dots = 10000;
    let dots_to_show = (base_dots as usize).min(max_dots).max(1);

    // Dot size shrinks as we add more dots for density
    let dot_size = if dots_to_show < 100 {
        2.5
    } else if dots_to_show < 500 {
        1.5
    } else if dots_to_show < 2000 {
        1.0
    } else {
        0.8
    };

    let usable_radius = inner_radius - 4.0;
    let golden_angle = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());

    // Store dot positions for drawing connections
    let mut dot_positions: Vec<Point> = Vec::with_capacity(dots_to_show);

    for i in 0..dots_to_show {
        let t = i as f32;
        let r = usable_radius * (t / dots_to_show as f32).sqrt();
        let theta = t * golden_angle;

        let dot_x = center.x + r * theta.cos();
        let dot_y = center.y + r * theta.sin();
        dot_positions.push(Point::new(dot_x, dot_y));
    }

    // Draw connections from nodes to dots (max 10000 lines total)
    let max_lines = 10000;
    let num_nodes = nodes.len();
    let num_dots = dot_positions.len();

    if num_nodes > 0 && num_dots > 0 {
        // Calculate how many connections we can draw
        // Each dot connects to 2-3 random nodes to show it's a coalition
        let connections_per_dot = if num_dots * 2 <= max_lines {
            2
        } else if num_dots <= max_lines {
            1
        } else {
            // Too many dots, only connect some of them
            0
        };

        let line_color = REED_COLOR.with_alpha(0.12);

        if connections_per_dot > 0 {
            let mut line_count = 0;
            for (i, dot) in dot_positions.iter().enumerate() {
                if line_count >= max_lines {
                    break;
                }
                // Connect to nodes based on dot index (deterministic pseudo-random)
                for j in 0..connections_per_dot {
                    let node_idx = (i * 7 + j * 13) % num_nodes;
                    let node = &nodes[node_idx];

                    // Draw thin line from node to dot
                    draw_thin_line(scene, *node, *dot, line_color);
                    line_count += 1;

                    if line_count >= max_lines {
                        break;
                    }
                }
            }
        } else if num_dots > 0 {
            // Connect only a subset of dots
            let dots_to_connect = max_lines.min(num_dots);
            let step = num_dots / dots_to_connect;
            let line_color = REED_COLOR.with_alpha(0.08);

            for i in (0..num_dots).step_by(step.max(1)) {
                let dot = &dot_positions[i];
                let node_idx = (i * 7) % num_nodes;
                let node = &nodes[node_idx];
                draw_thin_line(scene, *node, *dot, line_color);
            }
        }
    }

    // Draw dots on top of lines
    for (i, dot) in dot_positions.iter().enumerate() {
        let alpha = 0.7 + 0.3 * ((i % 7) as f32 / 7.0);
        scene.draw_quad(
            Quad::new(Bounds::new(
                dot.x - dot_size / 2.0,
                dot.y - dot_size / 2.0,
                dot_size,
                dot_size,
            ))
            .with_background(REED_COLOR.with_alpha(alpha)),
        );
    }

    // Draw nodes around perimeter on top
    let node_color = REED_COLOR;
    let node_radius = 4.0;
    for node in &nodes {
        draw_circle(scene, *node, node_radius, node_color);
    }
}

/// Draw a thin line (1px) between two points
fn draw_thin_line(scene: &mut Scene, from: Point, to: Point, color: Hsla) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let length = (dx * dx + dy * dy).sqrt();

    if length < 1.0 {
        return;
    }

    // Draw line as series of small quads
    let steps = (length / 2.0).max(1.0) as usize;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = from.x + dx * t;
        let y = from.y + dy * t;
        scene.draw_quad(
            Quad::new(Bounds::new(x - 0.5, y - 0.5, 1.0, 1.0))
                .with_background(color),
        );
    }
}

/// Draw comparison graph showing N² vs 2^N with proper log scale axes
fn draw_comparison_graph(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    bounds: Bounds,
    current_n: u32,
) {
    let margin_left = 55.0;
    let margin_right = 15.0;
    let margin_top = 10.0;
    let margin_bottom = 35.0;
    let graph_x = bounds.x() + margin_left;
    let graph_y = bounds.y() + margin_top;
    let graph_w = bounds.width() - margin_left - margin_right;
    let graph_h = bounds.height() - margin_top - margin_bottom;

    // Background
    scene.draw_quad(
        Quad::new(Bounds::new(graph_x, graph_y, graph_w, graph_h))
            .with_background(theme::bg::SURFACE.with_alpha(0.3))
            .with_border(theme::border::DEFAULT, 1.0),
    );

    // Log scale parameters
    // Y-axis: 10^0 to 10^15 (1 to 1 quadrillion)
    let log_min = 0.0_f64;  // 10^0 = 1
    let log_max = 15.0_f64; // 10^15

    // X-axis: 2 to 50 nodes
    let x_min = 2;
    let x_max = 50;

    // Helper to convert value to Y position (log scale)
    let value_to_y = |value: f64| -> f32 {
        if value <= 1.0 {
            return graph_y + graph_h;
        }
        let log_val = value.log10();
        let normalized = ((log_val - log_min) / (log_max - log_min)).clamp(0.0, 1.0);
        graph_y + graph_h - (normalized as f32) * graph_h
    };

    // Helper to convert N to X position
    let n_to_x = |n: u32| -> f32 {
        graph_x + ((n - x_min) as f32 / (x_max - x_min) as f32) * graph_w
    };

    // Draw Y-axis gridlines and labels (log scale: 10^0, 10^3, 10^6, 10^9, 10^12, 10^15)
    let y_ticks = [0, 3, 6, 9, 12, 15];
    let y_labels = ["1", "10^3", "10^6", "10^9", "10^12", "10^15"];
    for (i, &exp) in y_ticks.iter().enumerate() {
        let value = 10.0_f64.powi(exp);
        let y_pos = value_to_y(value);

        // Gridline
        scene.draw_quad(
            Quad::new(Bounds::new(graph_x, y_pos - 0.5, graph_w, 1.0))
                .with_background(theme::border::DEFAULT.with_alpha(0.3)),
        );

        // Label
        let label = y_labels[i];
        let label_width = text_system.measure(label, 9.0);
        let label_run = text_system.layout(
            label,
            Point::new(graph_x - label_width - 4.0, y_pos - 5.0),
            9.0,
            theme::text::MUTED,
        );
        scene.draw_text(label_run);
    }

    // Y-axis title
    let y_title = "Value (log)";
    let y_title_run = text_system.layout(
        y_title,
        Point::new(bounds.x() + 2.0, graph_y + graph_h / 2.0 - 30.0),
        9.0,
        theme::text::MUTED,
    );
    scene.draw_text(y_title_run);

    // Draw X-axis gridlines and labels
    let x_ticks = [2, 10, 20, 30, 40, 50];
    for &n in &x_ticks {
        let x_pos = n_to_x(n);

        // Gridline
        scene.draw_quad(
            Quad::new(Bounds::new(x_pos - 0.5, graph_y, 1.0, graph_h))
                .with_background(theme::border::DEFAULT.with_alpha(0.2)),
        );

        // Label
        let label = format!("{}", n);
        let label_width = text_system.measure(&label, 9.0);
        let label_run = text_system.layout(
            &label,
            Point::new(x_pos - label_width / 2.0, graph_y + graph_h + 4.0),
            9.0,
            theme::text::MUTED,
        );
        scene.draw_text(label_run);
    }

    // X-axis title
    let x_title = "N (nodes)";
    let x_title_width = text_system.measure(x_title, 10.0);
    let x_title_run = text_system.layout(
        x_title,
        Point::new(graph_x + graph_w / 2.0 - x_title_width / 2.0, graph_y + graph_h + 18.0),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(x_title_run);

    // Draw Metcalfe curve (N²) - yellow/orange - thicker line
    let metcalfe_color = METCALFE_COLOR;
    let mut prev_metcalfe: Option<(f32, f32)> = None;
    for n in x_min..=x_max {
        let value = (n as f64) * (n as f64);
        let px = n_to_x(n);
        let py = value_to_y(value);

        // Draw point
        scene.draw_quad(
            Quad::new(Bounds::new(px - 1.5, py - 1.5, 3.0, 3.0))
                .with_background(metcalfe_color),
        );

        // Connect to previous point
        if let Some((prev_x, prev_y)) = prev_metcalfe {
            draw_thick_line(scene, Point::new(prev_x, prev_y), Point::new(px, py), metcalfe_color, 2.0);
        }
        prev_metcalfe = Some((px, py));
    }

    // Draw Reed curve (2^N - N - 1) - green - thicker line, goes way higher
    let reed_color = REED_COLOR;
    let mut prev_reed: Option<(f32, f32)> = None;
    for n in x_min..=x_max {
        // Correct Reed's Law: 2^N - N - 1 (non-trivial subsets)
        let value = if n <= 50 {
            2.0_f64.powi(n as i32) - (n as f64) - 1.0
        } else {
            f64::MAX
        };

        let px = n_to_x(n);
        let py = value_to_y(value);

        // Draw point
        scene.draw_quad(
            Quad::new(Bounds::new(px - 1.5, py.max(graph_y) - 1.5, 3.0, 3.0))
                .with_background(reed_color),
        );

        // Connect to previous point
        if let Some((prev_x, prev_y)) = prev_reed {
            draw_thick_line(scene, Point::new(prev_x, prev_y.max(graph_y)), Point::new(px, py.max(graph_y)), reed_color, 2.0);
        }
        prev_reed = Some((px, py));
    }

    // Draw current N marker line
    let marker_x = n_to_x(current_n);
    scene.draw_quad(
        Quad::new(Bounds::new(marker_x - 1.0, graph_y, 2.0, graph_h))
            .with_background(theme::text::PRIMARY.with_alpha(0.5)),
    );

    // Current N label at top of marker
    let n_label = format!("N={}", current_n);
    let n_label_width = text_system.measure(&n_label, 10.0);
    let n_label_run = text_system.layout(
        &n_label,
        Point::new(marker_x - n_label_width / 2.0, graph_y - 14.0),
        10.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(n_label_run);

    // Legend in top-right corner
    let legend_x = graph_x + graph_w - 85.0;
    let legend_y = graph_y + 12.0;

    // Legend background
    scene.draw_quad(
        Quad::new(Bounds::new(legend_x - 6.0, legend_y - 4.0, 90.0, 32.0))
            .with_background(theme::bg::APP.with_alpha(0.8)),
    );

    // Metcalfe legend
    scene.draw_quad(
        Quad::new(Bounds::new(legend_x, legend_y + 2.0, 16.0, 3.0))
            .with_background(metcalfe_color),
    );
    let m_run = text_system.layout("N^2", Point::new(legend_x + 20.0, legend_y), 10.0, metcalfe_color);
    scene.draw_text(m_run);

    // Reed legend
    scene.draw_quad(
        Quad::new(Bounds::new(legend_x, legend_y + 16.0, 16.0, 3.0))
            .with_background(reed_color),
    );
    let r_run = text_system.layout("2^N-N-1", Point::new(legend_x + 20.0, legend_y + 14.0), 10.0, reed_color);
    scene.draw_text(r_run);
}

/// Draw a thick line between two points
fn draw_thick_line(scene: &mut Scene, from: Point, to: Point, color: Hsla, thickness: f32) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let length = (dx * dx + dy * dy).sqrt();

    if length < 0.5 {
        return;
    }

    let steps = (length / 1.5).max(1.0) as usize;
    let half_thick = thickness / 2.0;

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = from.x + dx * t;
        let y = from.y + dy * t;
        scene.draw_quad(
            Quad::new(Bounds::new(x - half_thick, y - half_thick, thickness, thickness))
                .with_background(color),
        );
    }
}

/// Draw a slider control
fn draw_slider(scene: &mut Scene, bounds: Bounds, value: u32, min: u32, max: u32) {
    let track_h = 4.0;
    let track_y = bounds.y() + (bounds.height() - track_h) / 2.0;

    // Track background
    scene.draw_quad(
        Quad::new(Bounds::new(bounds.x(), track_y, bounds.width(), track_h))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(2.0),
    );

    // Fill portion
    let fill_pct = (value - min) as f32 / (max - min) as f32;
    let fill_w = fill_pct * bounds.width();
    scene.draw_quad(
        Quad::new(Bounds::new(bounds.x(), track_y, fill_w, track_h))
            .with_background(METCALFE_COLOR)
            .with_corner_radius(2.0),
    );

    // Thumb
    let thumb_size = 14.0;
    let thumb_x = bounds.x() + fill_pct * (bounds.width() - thumb_size);
    let thumb_y = bounds.y() + (bounds.height() - thumb_size) / 2.0;
    scene.draw_quad(
        Quad::new(Bounds::new(thumb_x, thumb_y, thumb_size, thumb_size))
            .with_background(METCALFE_COLOR)
            .with_border(theme::text::PRIMARY, 1.0)
            .with_corner_radius(thumb_size / 2.0),
    );
}

/// Draw a line between two points
fn draw_line(scene: &mut Scene, from: Point, to: Point, color: Hsla) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let length = (dx * dx + dy * dy).sqrt();

    if length < 1.0 {
        return;
    }

    // For lines, we draw a series of small quads along the path
    let steps = (length / 2.0).max(1.0) as usize;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let x = from.x + dx * t;
        let y = from.y + dy * t;
        scene.draw_quad(
            Quad::new(Bounds::new(x - 0.5, y - 0.5, 1.0, 1.0))
                .with_background(color),
        );
    }
}

/// Draw a filled circle
fn draw_circle(scene: &mut Scene, center: Point, radius: f32, color: Hsla) {
    scene.draw_quad(
        Quad::new(Bounds::new(
            center.x - radius,
            center.y - radius,
            radius * 2.0,
            radius * 2.0,
        ))
        .with_background(color)
        .with_corner_radius(radius),
    );
}
