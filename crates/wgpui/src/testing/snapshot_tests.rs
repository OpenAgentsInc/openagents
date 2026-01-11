//! Snapshot tests for WGPUI scenes (d-013).

use crate::components::{Component, PaintContext, Text};
use crate::{Bounds, Quad, Scene, TextSystem, theme};
use insta::assert_snapshot;

fn scene_snapshot_string(scene: &Scene) -> String {
    let mut output = String::new();

    output.push_str("quads:\n");
    for (_, quad) in &scene.quads {
        output.push_str(&format!(
            "  bounds: {:?}, background: {:?}, border_width: {}\n",
            quad.bounds, quad.background, quad.border_width
        ));
    }

    output.push_str("text_runs:\n");
    for (_, run) in &scene.text_runs {
        output.push_str(&format!(
            "  origin: {:?}, glyphs: {}, font_size: {}\n",
            run.origin,
            run.glyphs.len(),
            run.font_size
        ));
    }

    output
}

#[test]
fn scene_snapshot() {
    let mut scene = Scene::new();
    let bounds = Bounds::new(0.0, 0.0, 240.0, 60.0);
    scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let mut text_system = TextSystem::new(1.0);
    let mut cx = PaintContext::new(&mut scene, &mut text_system, 1.0);
    let mut text = Text::new("Snapshot").font_size(theme::font_size::SM);
    text.paint(Bounds::new(12.0, 12.0, 200.0, 24.0), &mut cx);

    let snapshot = scene_snapshot_string(&scene);
    assert_snapshot!("scene_snapshot", snapshot);
}
