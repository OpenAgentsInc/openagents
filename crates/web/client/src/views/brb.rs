//! BRB (Be Right Back) simple page view

use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem, theme};
use wgpui::animation::AnimatorState;
use wgpui::components::Component;
use wgpui::components::hud::{DotsGrid, DotsOrigin};
use wgpui::PaintContext;
use wasm_bindgen::JsValue;

use crate::state::AppState;

/// Get the GitHub username from window.GITHUB_USERNAME
fn get_github_username() -> Option<String> {
    let window = web_sys::window()?;
    let value = js_sys::Reflect::get(&window, &JsValue::from_str("GITHUB_USERNAME")).ok()?;
    value.as_string()
}

/// Build the BRB page view
pub(crate) fn build_brb_page(
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

    // Whitelist of users who skip the "email when we launch" message
    // Currently empty - everyone sees the launch message
    let whitelist: &[&str] = &[];

    let username = get_github_username();
    let is_whitelisted = username.as_ref()
        .map(|u| whitelist.contains(&u.as_str()))
        .unwrap_or(false);

    // Show different text based on whitelist
    let (line1, line2) = if is_whitelisted {
        ("Join the waitlist", None)
    } else {
        ("Join the waitlist", Some("We'll email you when we launch in January."))
    };

    let font_size = 32.0;
    let small_font_size = 16.0;

    // Center the main text
    let text_width = text_system.measure(line1, font_size);
    let x = (width - text_width) / 2.0;
    let y = if line2.is_some() {
        height / 2.0 - font_size - 8.0
    } else {
        height / 2.0 - font_size / 2.0
    };

    let text_run = text_system.layout(line1, Point::new(x, y), font_size, theme::text::PRIMARY);
    scene.draw_text(text_run);

    // Draw second line if present
    if let Some(subtitle) = line2 {
        let subtitle_width = text_system.measure(subtitle, small_font_size);
        let subtitle_x = (width - subtitle_width) / 2.0;
        let subtitle_y = y + font_size + 16.0;
        let subtitle_run = text_system.layout(subtitle, Point::new(subtitle_x, subtitle_y), small_font_size, theme::text::MUTED);
        scene.draw_text(subtitle_run);
    }
}
