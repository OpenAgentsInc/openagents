mod landing;
mod repo_selector;
pub(crate) mod job_detail;

use wgpui::{Scene, TextSystem};

use crate::hud::draw_hud_view;
use crate::state::AppState;

// Re-export the main view functions
pub(crate) use landing::build_landing_page;
pub(crate) use repo_selector::build_repo_selector;

pub(crate) fn build_repo_view(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    draw_hud_view(scene, text_system, state, width, height, scale_factor);
}
