//! UI modules for Pylon desktop

mod fm_panel;
mod viz_panel;

use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::FmVizState;

// Panel background
fn panel_bg() -> Hsla {
    Hsla::new(220.0 / 360.0, 0.15, 0.08, 1.0)
}

// Accent colors
fn accent_cyan() -> Hsla {
    Hsla::new(180.0 / 360.0, 0.8, 0.5, 1.0)
}

fn accent_green() -> Hsla {
    Hsla::new(145.0 / 360.0, 0.7, 0.45, 1.0)
}

fn text_dim() -> Hsla {
    Hsla::new(0.0, 0.0, 0.5, 1.0)
}

pub fn build_pylon_ui(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &mut FmVizState,
    width: f32,
    height: f32,
) {
    // Background
    let bg = Hsla::new(220.0 / 360.0, 0.1, 0.05, 1.0);
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::ZERO,
            size: Size::new(width, height),
        })
        .with_background(bg),
    );

    let padding = 16.0;
    let content_width = width - padding * 2.0;

    // Title
    let title_y = padding;
    let run = text.layout("PYLON FM BRIDGE", Point::new(padding, title_y), 18.0, accent_cyan());
    scene.draw_text(run);

    let subtitle_y = title_y + 24.0;
    let run = text.layout(
        "Apple Foundation Models Inference",
        Point::new(padding, subtitle_y),
        12.0,
        text_dim(),
    );
    scene.draw_text(run);

    // Status bar
    let status_y = subtitle_y + 28.0;
    fm_panel::draw_status_bar(scene, text, state, padding, status_y, content_width);

    // Token stream panel
    let stream_y = status_y + 36.0;
    let stream_height = 200.0;
    fm_panel::draw_token_stream(scene, text, state, padding, stream_y, content_width, stream_height);

    // Session panel
    let session_y = stream_y + stream_height + 12.0;
    let session_height = 100.0;
    fm_panel::draw_session_panel(scene, text, state, padding, session_y, content_width, session_height);

    // Prompt input
    let input_y = session_y + session_height + 12.0;
    let input_height = 60.0;
    fm_panel::draw_prompt_input(scene, text, state, padding, input_y, content_width, input_height);

    // Viz dashboard
    let viz_y = input_y + input_height + 12.0;
    let viz_height = height - viz_y - padding;
    viz_panel::draw_viz_dashboard(scene, text, state, padding, viz_y, content_width, viz_height);
}
