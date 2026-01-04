//! Viz dashboard using viz crate primitives

use viz::fill::Bar;
use viz::grammar::{Heat, Palette};
use viz::heat::Rail;
use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::FmVizState;

use super::{accent_cyan, accent_green, panel_bg, text_dim};

pub fn draw_viz_dashboard(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &mut FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Panel background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        })
        .with_background(panel_bg()),
    );

    // Header
    let run = text.layout("VIZ DASHBOARD", Point::new(x + 12.0, y + 8.0), 11.0, text_dim());
    scene.draw_text(run);

    // Divider
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x + 12.0, y + 28.0),
            size: Size::new(width - 24.0, 1.0),
        })
        .with_background(text_dim().with_alpha(0.3)),
    );

    let content_y = y + 40.0;
    let col_width = (width - 48.0) / 3.0;

    // Create paint context
    let mut cx = PaintContext::new(scene, text, 1.0);

    // Column 1: Throughput Bar
    let bar_label_run = cx.text.layout("THROUGHPUT", Point::new(x + 12.0, content_y), 10.0, text_dim());
    cx.scene.draw_text(bar_label_run);

    let bar_bounds = Bounds {
        origin: Point::new(x + 12.0, content_y + 16.0),
        size: Size::new(col_width - 12.0, 20.0),
    };

    let mut bar = Bar::new()
        .with_value(state.tokens_per_sec / 20.0) // normalize to 0-1 (max 20 t/s)
        .with_colors(
            Hsla::new(0.0, 0.0, 0.15, 1.0),
            accent_cyan(),
        );
    bar.paint(bar_bounds, &mut cx);

    // Throughput value
    let tps_val = format!("{:.1} t/s", state.tokens_per_sec);
    let tps_run = cx.text.layout(&tps_val, Point::new(x + 12.0, content_y + 42.0), 11.0, accent_cyan());
    cx.scene.draw_text(tps_run);

    // Column 2: Token Count indicator
    let col2_x = x + 12.0 + col_width;
    let count_label_run = cx.text.layout("TOKENS", Point::new(col2_x, content_y), 10.0, text_dim());
    cx.scene.draw_text(count_label_run);

    let count_bounds = Bounds {
        origin: Point::new(col2_x, content_y + 16.0),
        size: Size::new(col_width - 12.0, 20.0),
    };

    // Normalized token count (max ~100 tokens for demo)
    let mut count_bar = Bar::new()
        .with_value((state.token_count as f32 / 50.0).min(1.0))
        .with_colors(
            Hsla::new(0.0, 0.0, 0.15, 1.0),
            accent_green(),
        );
    count_bar.paint(count_bounds, &mut cx);

    let count_val = format!("{} tokens", state.token_count);
    let count_run = cx.text.layout(&count_val, Point::new(col2_x, content_y + 42.0), 11.0, accent_green());
    cx.scene.draw_text(count_run);

    // Column 3: TTFT indicator
    let col3_x = x + 12.0 + col_width * 2.0;
    let ttft_label_run = cx.text.layout("TTFT", Point::new(col3_x, content_y), 10.0, text_dim());
    cx.scene.draw_text(ttft_label_run);

    if let Some(ttft) = state.ttft_ms {
        let ttft_val = format!("{}ms", ttft);
        let ttft_color = if ttft < 100 {
            accent_green()
        } else if ttft < 500 {
            Hsla::new(45.0 / 360.0, 1.0, 0.5, 1.0) // yellow
        } else {
            Hsla::new(0.0, 0.9, 0.5, 1.0) // red
        };
        let ttft_run = cx.text.layout(&ttft_val, Point::new(col3_x, content_y + 20.0), 18.0, ttft_color);
        cx.scene.draw_text(ttft_run);
    }

    // Token History Rail (spans full width below the columns)
    let rail_y = content_y + 70.0;
    let rail_label_run = cx.text.layout("TOKEN HISTORY", Point::new(x + 12.0, rail_y), 10.0, text_dim());
    cx.scene.draw_text(rail_label_run);

    let rail_bounds = Bounds {
        origin: Point::new(x + 12.0, rail_y + 16.0),
        size: Size::new(width - 24.0, 24.0),
    };

    let mut rail = Rail::new(state.token_history.len());
    // Use a cyan-green gradient palette
    rail.set_palette(Palette {
        colors: vec![
            [0.1, 0.15, 0.2, 1.0],  // dark blue
            [0.3, 0.7, 0.7, 1.0],   // cyan
            [0.4, 0.8, 0.5, 1.0],   // green
        ],
    });

    // Push all history values
    for &val in &state.token_history {
        rail.push(val);
    }

    rail.paint(rail_bounds, &mut cx);

    // Stats row at bottom
    let stats_y = rail_y + 50.0;
    if stats_y + 20.0 < y + height {
        let ping_text = format!(
            "PING: {}ms",
            state.ping_latency_ms.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string())
        );
        let ping_run = cx.text.layout(&ping_text, Point::new(x + 12.0, stats_y), 10.0, text_dim());
        cx.scene.draw_text(ping_run);

        let status_text = if state.model_available { "MODEL: OK" } else { "MODEL: --" };
        let status_run = cx.text.layout(status_text, Point::new(x + 120.0, stats_y), 10.0, text_dim());
        cx.scene.draw_text(status_run);
    }
}
