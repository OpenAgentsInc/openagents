use crate::state::Storybook;
use wgpui::{Bounds, PaintContext, Scene, TextSystem};

pub fn build_storybook_scene(
    story: &mut Storybook,
    scene: &mut Scene,
    text_system: &mut TextSystem,
    bounds: Bounds,
    scale_factor: f32,
) {
    story.paint(
        bounds,
        &mut PaintContext::new(scene, text_system, scale_factor),
    );
}
