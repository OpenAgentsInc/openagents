//! FRLM (Fracking Apple Silicon) power comparison page view
//!
//! Companion visualization for "Fracking Apple Silicon" video showing
//! electrical power capacity comparison across AI compute paradigms.

use wgpui::animation::AnimatorState;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::components::Component;
use wgpui::PaintContext;
use wgpui::{theme, Bounds, Hsla, Point, Quad, Scene, TextSystem};

use crate::state::AppState;

/// Boring gray for typical datacenter (understated)
const DATACENTER_COLOR: Hsla = Hsla::new(0.0, 0.0, 0.45, 1.0);

/// Light blue for Stargate
const STARGATE_COLOR: Hsla = Hsla::new(0.556, 0.7, 0.6, 1.0);

/// Orange for Apple Silicon swarm (same as /gfn)
const APPLE_COLOR: Hsla = Hsla::new(0.1, 1.0, 0.5, 1.0);

/// Power values in GW
const DATACENTER_GW: f32 = 0.10;
const STARGATE_GW: f32 = 1.20;
const APPLE_GW: f32 = 5.50;
const MAX_GW: f32 = 6.0;

/// Build the FRLM power comparison page view
pub(crate) fn build_frlm_page(
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

    // Dots grid background
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 1.0, 0.25))
        .distance(36.0)
        .size(1.0)
        .origin(DotsOrigin::Center);
    state.dots_grid.update(AnimatorState::Entered);
    state.dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // Layout calculations
    let padding = 24.0;
    let content_width = (width - padding * 2.0).min(1000.0);
    let content_x = (width - content_width) / 2.0;

    // Start frame animation
    if !state.frlm.frame_started {
        state.frlm.frame_started = true;
    }
    let frame_progress = state.frlm.frame_animator.update(AnimatorState::Entering);

    // Main card dimensions
    let card_y = padding;
    let card_height = height - padding * 2.0;
    let card_bounds = Bounds::new(content_x, card_y, content_width, card_height);

    // Draw main frame with corners
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    let mut frame = Frame::corners()
        .line_color(Hsla::new(0.0, 0.0, 1.0, 0.8))
        .bg_color(Hsla::new(0.0, 0.0, 0.0, 0.5))
        .glow_color(Hsla::new(0.0, 0.0, 1.0, 0.12))
        .border_color(Hsla::new(0.0, 0.0, 1.0, 0.1))
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
    state.frlm.content_bounds =
        Bounds::new(inner_x, card_y + inner_padding, inner_width + scrollbar_width, visible_height);

    // Push clip for scrollable content
    scene.push_clip(Bounds::new(
        inner_x - 4.0,
        card_y + inner_padding,
        inner_width + scrollbar_width + 8.0,
        visible_height,
    ));

    // Apply scroll offset
    let scroll_offset = state.frlm.scroll_offset;
    let mut y = card_y + inner_padding - scroll_offset;

    // Title
    let title = "POWER AT SCALE";
    let title_size = 28.0;
    let title_width = text_system.measure(title, title_size);
    let title_x = inner_x + (inner_width - title_width) / 2.0;
    let title_run = text_system.layout(title, Point::new(title_x, y), title_size, theme::text::PRIMARY);
    scene.draw_text(title_run);
    y += title_size + 8.0;

    // Subtitle
    let subtitle = "AI Data Center vs OpenAI Stargate vs Apple Silicon Swarm";
    let subtitle_size = 13.0;
    let subtitle_width = text_system.measure(subtitle, subtitle_size);
    let subtitle_x = inner_x + (inner_width - subtitle_width) / 2.0;
    let subtitle_run =
        text_system.layout(subtitle, Point::new(subtitle_x, y), subtitle_size, theme::text::MUTED);
    scene.draw_text(subtitle_run);
    y += subtitle_size + 40.0;

    // Bar chart section
    let chart_height = 280.0;
    let chart_top = y;
    let chart_bottom = y + chart_height;

    // Y-axis labels
    let y_axis_x = inner_x;
    let label_size = 11.0;

    // Draw Y-axis scale (0, 1, 2, 3, 4, 5, 6 GW)
    for i in 0..=6 {
        let gw = i as f32;
        let label_y = chart_bottom - (gw / MAX_GW) * chart_height - 4.0;
        let label = format!("{:.0}", gw);
        let label_run =
            text_system.layout(&label, Point::new(y_axis_x, label_y), label_size, theme::text::MUTED);
        scene.draw_text(label_run);

        // Grid line
        if i > 0 {
            let line_y = chart_bottom - (gw / MAX_GW) * chart_height;
            scene.draw_quad(
                Quad::new(Bounds::new(y_axis_x + 20.0, line_y, inner_width - 30.0, 1.0))
                    .with_background(Hsla::new(0.0, 0.0, 1.0, 0.08)),
            );
        }
    }

    // Y-axis label
    let y_label = "GW";
    let y_label_run =
        text_system.layout(y_label, Point::new(y_axis_x, chart_top - 16.0), label_size, theme::text::MUTED);
    scene.draw_text(y_label_run);

    // Bar dimensions
    let bar_area_x = inner_x + 40.0;
    let bar_area_width = inner_width - 50.0;
    let bar_width = 100.0;
    let bar_gap = (bar_area_width - bar_width * 3.0) / 4.0;

    // Draw bars (left to right: Datacenter, Stargate, Apple)
    let bars = [
        (DATACENTER_GW, DATACENTER_COLOR, "Typical DC", "100 MW"),
        (STARGATE_GW, STARGATE_COLOR, "Stargate", "1.2 GW"),
        (APPLE_GW, APPLE_COLOR, "110M Macs", "5.5 GW"),
    ];

    for (i, (gw, color, label, value_label)) in bars.iter().enumerate() {
        let bar_x = bar_area_x + bar_gap + (bar_width + bar_gap) * i as f32;
        let bar_height = (*gw / MAX_GW) * chart_height;
        let bar_y = chart_bottom - bar_height;

        // Store bar bounds for hover detection
        let bar_bounds = Bounds::new(bar_x, bar_y, bar_width, bar_height);
        state.frlm.bar_bounds[i] = bar_bounds;

        // Hover effect
        let is_hovered = state.frlm.bar_hover_index == Some(i);
        let bar_color = if is_hovered {
            (*color).lighten(0.1)
        } else {
            *color
        };

        // Draw bar
        scene.draw_quad(Quad::new(bar_bounds).with_background(bar_color));

        // Value label above bar
        let value_size = 14.0;
        let value_width = text_system.measure(value_label, value_size);
        let value_x = bar_x + (bar_width - value_width) / 2.0;
        let value_y = bar_y - 20.0;
        let value_run = text_system.layout(value_label, Point::new(value_x, value_y), value_size, *color);
        scene.draw_text(value_run);

        // Label below bar
        let label_width = text_system.measure(label, label_size);
        let label_x = bar_x + (bar_width - label_width) / 2.0;
        let label_run =
            text_system.layout(label, Point::new(label_x, chart_bottom + 8.0), label_size, theme::text::MUTED);
        scene.draw_text(label_run);
    }

    y = chart_bottom + 50.0;

    // Key insight box
    let insight_box_height = 60.0;
    let insight_bounds = Bounds::new(inner_x, y, inner_width, insight_box_height);
    scene.draw_quad(
        Quad::new(insight_bounds)
            .with_background(Hsla::new(0.0, 0.0, 1.0, 0.1))
            .with_border(Hsla::new(0.0, 0.0, 1.0, 0.9), 1.0),
    );

    let insight_text = "Stargate (1.2 GW) = 24 million Macs at 50W each";
    let insight_size = 15.0;
    let insight_width = text_system.measure(insight_text, insight_size);
    let insight_x = inner_x + (inner_width - insight_width) / 2.0;
    let insight_run =
        text_system.layout(insight_text, Point::new(insight_x, y + 12.0), insight_size, STARGATE_COLOR);
    scene.draw_text(insight_run);

    let insight_sub = "Apple's installed base: 110M Macs = 4.6x Stargate capacity";
    let insight_sub_size = 12.0;
    let insight_sub_width = text_system.measure(insight_sub, insight_sub_size);
    let insight_sub_x = inner_x + (inner_width - insight_sub_width) / 2.0;
    let insight_sub_run = text_system.layout(
        insight_sub,
        Point::new(insight_sub_x, y + 34.0),
        insight_sub_size,
        theme::text::MUTED,
    );
    scene.draw_text(insight_sub_run);

    y += insight_box_height + 32.0;

    // Two-column content section
    let col_width = (inner_width - 24.0) / 2.0;
    let col_gap = 24.0;
    let section_size = 12.0;
    let header_size = 13.0;

    // Left column: The Numbers
    let left_x = inner_x;
    let mut left_y = y;

    let header1 = "THE NUMBERS";
    let header1_run =
        text_system.layout(header1, Point::new(left_x, left_y), header_size, theme::text::PRIMARY);
    scene.draw_text(header1_run);
    left_y += header_size + 12.0;

    let numbers = [
        "Typical hyperscale DC: ~100 MW",
        "Stargate Abilene: 1.2 GW capacity",
        "110M Apple Silicon Macs x 50W = 5.5 GW",
    ];
    for line in &numbers {
        let run = text_system.layout(line, Point::new(left_x, left_y), section_size, theme::text::MUTED);
        scene.draw_text(run);
        left_y += section_size + 6.0;
    }

    // Right column: The Insight
    let right_x = inner_x + col_width + col_gap;
    let mut right_y = y;

    let header2 = "THE INSIGHT";
    let header2_run =
        text_system.layout(header2, Point::new(right_x, right_y), header_size, theme::text::PRIMARY);
    scene.draw_text(header2_run);
    right_y += header_size + 12.0;

    let insights = [
        "Apple's fleet already exists",
        "No new infrastructure required",
        "Distributed, always available",
        "Already paid for by consumers",
    ];
    for line in &insights {
        let run = text_system.layout(line, Point::new(right_x, right_y), section_size, theme::text::MUTED);
        scene.draw_text(run);
        right_y += section_size + 6.0;
    }

    y = left_y.max(right_y) + 24.0;

    // Second row: FRLM | Sources
    let mut left_y = y;
    let mut right_y = y;

    let header3 = "FRLM: FEDERATED RECURSION";
    let header3_run =
        text_system.layout(header3, Point::new(left_x, left_y), header_size, APPLE_COLOR);
    scene.draw_text(header3_run);
    left_y += header_size + 12.0;

    let frlm_points = [
        "Engine-driven orchestration",
        "Works with on-device Apple FM",
        "Simple prompts x parallel chunks",
        "Trace-native verification",
    ];
    for line in &frlm_points {
        let run = text_system.layout(line, Point::new(left_x, left_y), section_size, theme::text::MUTED);
        scene.draw_text(run);
        left_y += section_size + 6.0;
    }

    let header4 = "SOURCES";
    let header4_run =
        text_system.layout(header4, Point::new(right_x, right_y), header_size, theme::text::PRIMARY);
    scene.draw_text(header4_run);
    right_y += header_size + 12.0;

    let sources = [
        "ASME/IEA: Hyperscale DC ~100MW",
        "DatacenterDynamics: Stargate 1.2 GW",
        "Apple: ~110M active Macs (est.)",
        "Power draw: 30-60W inference (50W avg)",
    ];
    for line in &sources {
        let run = text_system.layout(line, Point::new(right_x, right_y), section_size, theme::text::MUTED);
        scene.draw_text(run);
        right_y += section_size + 6.0;
    }

    y = left_y.max(right_y) + 32.0;

    // Footer
    let footer = "openagents.com/frack";
    let footer_size = 11.0;
    let footer_width = text_system.measure(footer, footer_size);
    let footer_x = inner_x + (inner_width - footer_width) / 2.0;
    let footer_run =
        text_system.layout(footer, Point::new(footer_x, y), footer_size, theme::text::MUTED);
    scene.draw_text(footer_run);

    y += footer_size + inner_padding;

    // Store total content height for scrolling
    state.frlm.content_height = y - (card_y + inner_padding) + scroll_offset;

    // Pop clip
    scene.pop_clip();

    // Draw scrollbar if needed
    if state.frlm.content_height > visible_height {
        let scrollbar_x = content_x + content_width - inner_padding - scrollbar_width + 4.0;
        let scrollbar_track_height = visible_height - 8.0;
        let scrollbar_y = card_y + inner_padding + 4.0;

        // Track
        scene.draw_quad(
            Quad::new(Bounds::new(scrollbar_x, scrollbar_y, 6.0, scrollbar_track_height))
                .with_background(Hsla::new(0.0, 0.0, 1.0, 0.1)),
        );

        // Thumb
        let visible_ratio = visible_height / state.frlm.content_height;
        let thumb_height = (scrollbar_track_height * visible_ratio).max(30.0);
        let scroll_ratio = state.frlm.scroll_offset / (state.frlm.content_height - visible_height);
        let thumb_y = scrollbar_y + scroll_ratio * (scrollbar_track_height - thumb_height);

        scene.draw_quad(
            Quad::new(Bounds::new(scrollbar_x, thumb_y, 6.0, thumb_height))
                .with_background(Hsla::new(0.0, 0.0, 1.0, 0.3)),
        );
    }
}

/// Handle mouse move for bar hover detection
pub(crate) fn handle_frlm_mouse_move(state: &mut AppState, x: f32, y: f32) {
    let mut hovered = None;
    for (i, bounds) in state.frlm.bar_bounds.iter().enumerate() {
        if bounds.contains(Point::new(x, y)) {
            hovered = Some(i);
            break;
        }
    }
    state.frlm.bar_hover_index = hovered;
}
