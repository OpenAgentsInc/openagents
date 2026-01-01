//! 2026 page view - Key themes and links

use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem, theme};
use wgpui::animation::AnimatorState;
use wgpui::components::Component;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::PaintContext;

use crate::state::AppState;

/// Orange accent color
const ACCENT_COLOR: Hsla = Hsla::new(0.1, 1.0, 0.5, 1.0);

/// The 6 bullet points with their links
const ITEMS: [(&str, &[&str]); 6] = [
    ("Local AI", &[
        "https://x.com/alexocheema/status/2006535639894995138",
        "https://x.com/OpenAgentsInc/status/1986655639431500034",
    ]),
    ("Swarm AI", &[
        "https://x.com/OpenAgentsInc/status/1998437331070505222",
        "https://x.com/OpenAgentsInc/status/2005402985166500320",
    ]),
    ("Open > Closed", &[
        "https://x.com/OpenAgentsInc/status/2003872403185938599",
    ]),
    ("Agents > Models", &[
        "https://x.com/OpenAgentsInc/status/1991491550904480064",
    ]),
    ("Autopilots", &[
        "https://x.com/OpenAgentsInc/status/2003362087955730508",
    ]),
    ("Agent Network(s)", &[
        "https://x.com/OpenAgentsInc/status/1989473563066376585",
        "https://x.com/OpenAgentsInc/status/2002621049616179223",
        "https://github.com/OpenAgentsInc/openagents/blob/main/docs/research/openagents-vs-distributional-agi-safety.md",
        "https://chatgpt.com/share/6956b860-9288-8011-b67d-c78b64fceb49",
    ]),
];

/// Build the 2026 page view
pub(crate) fn build_2026_page(
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
    let content_width = (width - padding * 2.0).min(700.0);
    let content_x = (width - content_width) / 2.0;

    // Start frame animation
    if !state.y2026.frame_started {
        state.y2026.frame_started = true;
    }
    let frame_progress = state.y2026.frame_animator.update(AnimatorState::Entering);

    // Main card dimensions
    let card_y = padding;
    let card_height = height - padding * 2.0;
    let card_bounds = Bounds::new(content_x, card_y, content_width, card_height);

    // Draw main frame with corners
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    let mut frame = Frame::corners()
        .line_color(Hsla::new(0.0, 0.0, 1.0, 0.8))
        .bg_color(Hsla::new(0.0, 0.0, 0.0, 0.4))
        .glow_color(Hsla::new(0.0, 0.0, 1.0, 0.15))
        .border_color(Hsla::new(0.0, 0.0, 1.0, 0.1))
        .stroke_width(1.0)
        .corner_length(30.0)
        .animation_progress(frame_progress);
    frame.paint(card_bounds, &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // Inner content padding
    let inner_padding = 40.0;
    let inner_x = content_x + inner_padding;
    let _inner_width = content_width - inner_padding * 2.0;

    // Title
    let mut y = card_y + inner_padding + 20.0;
    let title = "2026";
    let title_run = text_system.layout(title, Point::new(inner_x, y), 48.0, ACCENT_COLOR);
    scene.draw_text(title_run);
    y += 70.0;

    // Bullet points
    let line_height = 44.0;
    let bullet_size = 14.0;
    let link_size = 12.0;

    // Store link bounds for click detection
    state.y2026.link_bounds.clear();

    for (title, links) in ITEMS.iter() {
        // Bullet point
        let bullet_x = inner_x;
        let bullet_y = y + 4.0;
        scene.draw_quad(
            Quad::new(Bounds::new(bullet_x, bullet_y, 6.0, 6.0))
                .with_background(ACCENT_COLOR)
                .with_corner_radius(3.0),
        );

        // Title text
        let title_x = inner_x + 20.0;
        let title_run = text_system.layout(title, Point::new(title_x, y), bullet_size, theme::text::PRIMARY);
        scene.draw_text(title_run);

        // Links as [1] [2] [3] after the title
        let mut link_x = title_x + (title.len() as f32 * 8.0) + 16.0;
        for (i, url) in links.iter().enumerate() {
            let link_text = format!("[{}]", i + 1);
            let link_run = text_system.layout(&link_text, Point::new(link_x, y + 1.0), link_size, ACCENT_COLOR);
            scene.draw_text(link_run);

            // Store bounds for click detection
            let link_bounds = Bounds::new(link_x - 4.0, y - 2.0, 24.0, 20.0);
            state.y2026.link_bounds.push((link_bounds, url.to_string()));

            link_x += 28.0;
        }

        y += line_height;
    }

    // Clear other page state
    state.button_bounds = Bounds::ZERO;
    state.left_cta_bounds = Bounds::ZERO;
    state.right_cta_bounds = Bounds::ZERO;
}
