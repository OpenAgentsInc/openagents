mod brb;
mod gfn;
mod gptoss;
mod landing;
mod ml_inference;
mod repo_selector;
mod y2026;
pub(crate) mod job_detail;

use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem, theme};
use wgpui::animation::AnimatorState;
use wgpui::components::Component;
use wgpui::components::hud::{DotsGrid, DotsOrigin};
use wgpui::PaintContext;

use crate::state::AppState;

// Re-export the main view functions
pub(crate) use brb::build_brb_page;
pub(crate) use gfn::build_gfn_page;
pub(crate) use gptoss::build_gptoss_page;
pub(crate) use landing::build_landing_page;
pub(crate) use ml_inference::build_ml_inference_page;
pub(crate) use repo_selector::build_repo_selector;
pub(crate) use y2026::build_2026_page;

pub(crate) fn build_repo_view(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    // Dots grid background
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 1.0, 0.15))
        .distance(36.0)
        .size(1.0)
        .origin(DotsOrigin::Center);
    state.dots_grid.update(AnimatorState::Entered);
    state.dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // Repo name (top left)
    if let Some(ctx) = &state.hud_context {
        let repo_name = format!("{}/{}", ctx.username, ctx.repo);
        let repo_run = text_system.layout(
            &repo_name,
            Point::new(24.0, 24.0),
            14.0,
            theme::text::PRIMARY,
        );
        scene.draw_text(repo_run);
    }

    // Logout button (top right)
    let logout_text = "Log out";
    let logout_size = 12.0;
    let logout_width = text_system.measure(logout_text, logout_size) + 16.0;
    let logout_x = width - 24.0 - logout_width;
    let logout_y = 20.0;
    state.button_bounds = Bounds::new(logout_x, logout_y, logout_width, 24.0);

    let border_color = if state.button_hovered {
        theme::status::ERROR
    } else {
        theme::status::ERROR.with_alpha(0.7)
    };

    scene.draw_quad(
        Quad::new(state.button_bounds)
            .with_background(Hsla::new(0.0, 0.0, 0.0, 0.0))
            .with_border(border_color, 1.0),
    );

    let logout_run = text_system.layout(
        logout_text,
        Point::new(logout_x + 8.0, logout_y + 4.0),
        logout_size,
        border_color,
    );
    scene.draw_text(logout_run);

    // Clear all HUD layout bounds to prevent ghost button clicks
    state.hud_layout.thread_bounds = Bounds::ZERO;
    state.hud_layout.code_bounds = Bounds::ZERO;
    state.hud_layout.terminal_bounds = Bounds::ZERO;
    state.hud_layout.metrics_bounds = Bounds::ZERO;
    state.hud_layout.wallet_bounds = Bounds::ZERO;
    state.hud_layout.start_form_bounds = Bounds::ZERO;
    state.hud_layout.start_prompt_bounds = Bounds::ZERO;
    state.hud_layout.start_button_bounds = Bounds::ZERO;
    state.hud_layout.share_button_bounds = Bounds::ZERO;
    state.hud_layout.share_panel_bounds = Bounds::ZERO;
    state.hud_layout.copy_url_bounds = Bounds::ZERO;
    state.hud_layout.copy_embed_bounds = Bounds::ZERO;
    state.hud_layout.status_bounds = Bounds::ZERO;
    state.hud_layout.settings_public_bounds = Bounds::ZERO;
    state.hud_layout.settings_embed_bounds = Bounds::ZERO;

    // Clear landing page bounds to prevent ghost hover/clicks
    state.left_cta_bounds = Bounds::ZERO;
    state.right_cta_bounds = Bounds::ZERO;
    state.landing_issue_bounds = Bounds::ZERO;
    state.left_cta_hovered = false;
    state.right_cta_hovered = false;

    // Clear repo selector bounds
    state.repo_bounds.clear();
    state.hovered_repo_idx = None;

    // Clear DVM/Nostr bounds
    state.dvm_tab_bounds = [Bounds::ZERO, Bounds::ZERO];
    state.dvm_content_bounds = Bounds::ZERO;
    state.nip90_event_bounds.clear();
    state.global_feed_bounds = Bounds::ZERO;

    // Draw autopilot chat overlay (on top of everything)
    if state.autopilot_chat.visible {
        state.autopilot_chat.calculate_bounds(width, height);
        let mut cx = PaintContext::new(scene, text_system, scale_factor);
        state.autopilot_chat.paint(&mut cx);
    }

    // Draw Claude chat overlay (highest priority)
    if state.claude_chat.visible {
        state.claude_chat.calculate_bounds(width, height);
        let mut cx = PaintContext::new(scene, text_system, scale_factor);
        state.claude_chat.paint(&mut cx);
    }
}
