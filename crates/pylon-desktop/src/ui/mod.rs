//! UI modules for Pylon desktop
//!
//! Layout with FM interface + Nostr panels:
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │ PYLON                              FM: OK    ● ONLINE   ▲ 47 credits        │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │ TOKEN STREAM                                                    12.5 t/s    │
//! │ Hello! I'm an AI assistant. How can I help you today?                       │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │ PROMPT > What is 2+2?_                                                      │
//! ├───────────────────────────────────┬─────────────────────────────────────────┤
//! │ JOBS                              │ #openagents-providers                   │
//! │ ▶ 5050 from alice...  SERVING     │ [bob] Just hit 100 jobs today           │
//! ├───────────────────────────────────┴─────────────────────────────────────────┤
//! │ TOKEN RAIL  ▓▓▓▓▓▓▓▓▓▒▒▒▒░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░                   │
//! └─────────────────────────────────────────────────────────────────────────────┘

mod chat_panel;
mod fm_panel;
mod header;
mod jobs_panel;

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
    // Full-screen dark background
    let bg = Hsla::new(220.0 / 360.0, 0.1, 0.05, 1.0);
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::ZERO,
            size: Size::new(width, height),
        })
        .with_background(bg),
    );

    let padding = 4.0;
    let gap = 4.0;

    // Header bar (full width)
    let header_height = 40.0;
    header::draw_header(scene, text, state, padding, padding, width - padding * 2.0, header_height);

    let mut y = padding + header_height + gap;

    // Token stream panel (FM output)
    let stream_height = 180.0;
    fm_panel::draw_token_stream(scene, text, state, padding, y, width - padding * 2.0, stream_height);
    y += stream_height + gap;

    // Prompt input
    let input_height = 50.0;
    fm_panel::draw_prompt_input(scene, text, state, padding, y, width - padding * 2.0, input_height);
    y += input_height + gap;

    // Footer height
    let footer_height = 60.0;
    let nostr_height = height - y - footer_height - padding - gap;

    // Split panels for Nostr (Jobs + Chat) - only if there's room
    if nostr_height > 100.0 {
        let panel_width = (width - padding * 2.0 - gap) / 2.0;

        // Left panel: Jobs
        jobs_panel::draw_jobs_panel(scene, text, state, padding, y, panel_width, nostr_height);

        // Right panel: Chat
        chat_panel::draw_chat_panel(scene, text, state, padding + panel_width + gap, y, panel_width, nostr_height);

        y += nostr_height + gap;
    }

    // Footer: Token rail
    draw_footer(scene, text, state, padding, y, width - padding * 2.0, footer_height);
}

/// Compact footer with token rail and stats
fn draw_footer(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &mut FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        })
        .with_background(panel_bg()),
    );

    // Token Rail header
    let run = text.layout("TOKEN RAIL", Point::new(x + 12.0, y + 6.0), 10.0, text_dim());
    scene.draw_text(run);

    // Tokens per second (right side)
    let tps_text = format!("{:.1} t/s", state.tokens_per_sec);
    let tps_width = text.measure(&tps_text, 12.0);
    let run = text.layout(&tps_text, Point::new(x + width - tps_width - 12.0, y + 4.0), 12.0, accent_cyan());
    scene.draw_text(run);

    // Token count
    let count_text = format!("{} tokens", state.token_count);
    let count_width = text.measure(&count_text, 10.0);
    let run = text.layout(&count_text, Point::new(x + width - tps_width - count_width - 30.0, y + 6.0), 10.0, text_dim());
    scene.draw_text(run);

    // Token history rail
    let rail_y = y + 22.0;
    let rail_height = height - 28.0;
    let rail_width = width - 24.0;

    // Rail background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x + 12.0, rail_y),
            size: Size::new(rail_width, rail_height),
        })
        .with_background(Hsla::new(0.0, 0.0, 0.1, 1.0)),
    );

    // Draw token history bars
    let bar_count = state.token_history.len();
    if bar_count > 0 {
        let bar_width = rail_width / bar_count as f32;
        for (i, &value) in state.token_history.iter().enumerate() {
            let bar_h = (value * rail_height).min(rail_height);
            let bar_x = x + 12.0 + i as f32 * bar_width;
            let bar_y = rail_y + rail_height - bar_h;

            if bar_h > 0.5 {
                let intensity = value.min(1.0);
                let color = Hsla::new(
                    180.0 / 360.0 + intensity * 35.0 / 360.0,
                    0.7,
                    0.4 + intensity * 0.2,
                    0.8,
                );
                scene.draw_quad(
                    Quad::new(Bounds {
                        origin: Point::new(bar_x, bar_y),
                        size: Size::new(bar_width - 1.0, bar_h),
                    })
                    .with_background(color),
                );
            }
        }
    }
}
