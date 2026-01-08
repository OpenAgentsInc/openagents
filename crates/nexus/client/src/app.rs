//! Nexus HUD application
//!
//! Entry point and rendering for the Nexus relay statistics dashboard.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use wgpui::animation::AnimatorState;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::components::Component;
use wgpui::{
    run_animation_loop, setup_resize_observer, Bounds, Hsla, Platform, Point, Quad, Scene,
    TextSystem, WebPlatform, theme,
};
use wgpui::PaintContext;

use crate::state::{ConnectionStatus, NexusState, RelayStats};

/// Entry point for the Nexus HUD
#[wasm_bindgen]
pub async fn start_nexus_hud(canvas_id: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"Nexus HUD initializing...".into());

    let platform = WebPlatform::init(canvas_id)
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let platform = Rc::new(RefCell::new(platform));
    let state = Rc::new(RefCell::new(NexusState::default()));

    platform.borrow_mut().handle_resize();

    // Set up resize observer
    {
        let platform_clone = platform.clone();
        let canvas = platform.borrow().canvas().clone();
        setup_resize_observer(&canvas, move || {
            platform_clone.borrow_mut().handle_resize();
        });
    }

    // Start stats polling
    start_stats_polling(state.clone());

    // Run animation loop
    let platform_clone = platform.clone();
    let state_clone = state.clone();
    run_animation_loop(move || {
        let mut platform = platform_clone.borrow_mut();
        let mut state = state_clone.borrow_mut();

        let size = platform.logical_size();
        let width = size.width;
        let height = size.height;
        let scale_factor = platform.scale_factor();

        let mut scene = Scene::new();
        render_hud(&mut state, &mut scene, platform.text_system(), width, height, scale_factor);

        if let Err(e) = platform.render_scene(&scene) {
            web_sys::console::error_1(&format!("Render error: {}", e).into());
        }
    });

    Ok(())
}

/// Start periodic stats fetching
fn start_stats_polling(state: Rc<RefCell<NexusState>>) {
    // Initial fetch
    fetch_stats(state.clone());

    // Set up interval for periodic fetching
    let state_clone = state.clone();
    let closure = Closure::wrap(Box::new(move || {
        fetch_stats(state_clone.clone());
    }) as Box<dyn Fn()>);

    let window = web_sys::window().unwrap();
    window
        .set_interval_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            5000, // 5 seconds
        )
        .unwrap();

    closure.forget();
}

/// Fetch stats from /api/stats
fn fetch_stats(state: Rc<RefCell<NexusState>>) {
    spawn_local(async move {
        state.borrow_mut().connection_status = ConnectionStatus::Connecting;

        let window = web_sys::window().unwrap();
        let promise = window.fetch_with_str("/api/stats");
        let result = wasm_bindgen_futures::JsFuture::from(promise).await;

        match result {
            Ok(resp_value) => {
                let resp: web_sys::Response = resp_value.dyn_into().unwrap();
                if resp.ok() {
                    let json_promise = resp.json().unwrap();
                    match wasm_bindgen_futures::JsFuture::from(json_promise).await {
                        Ok(json_value) => {
                            let stats: RelayStats =
                                serde_wasm_bindgen::from_value(json_value).unwrap_or_default();
                            let mut state = state.borrow_mut();
                            state.stats = stats;
                            state.connection_status = ConnectionStatus::Connected;
                            state.fetch_error = None;
                            state.fetch_count += 1;
                            state.last_fetch_time = js_sys::Date::now() as u64;
                        }
                        Err(e) => {
                            let mut state = state.borrow_mut();
                            state.connection_status = ConnectionStatus::Error;
                            state.fetch_error = Some(format!("JSON parse error: {:?}", e));
                        }
                    }
                } else {
                    let mut state = state.borrow_mut();
                    state.connection_status = ConnectionStatus::Error;
                    state.fetch_error = Some(format!("HTTP {}", resp.status()));
                }
            }
            Err(e) => {
                let mut state = state.borrow_mut();
                state.connection_status = ConnectionStatus::Error;
                state.fetch_error = Some(format!("Fetch error: {:?}", e));
            }
        }
    });
}

/// Render the HUD
fn render_hud(
    state: &mut NexusState,
    scene: &mut Scene,
    text_system: &mut TextSystem,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    let bounds = Bounds::new(0.0, 0.0, width, height);

    // Background
    scene.draw_quad(Quad::new(bounds).with_background(Hsla::from_hex(0x0a0a0a)));

    // Dots grid background
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    let mut dots = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 1.0, 0.25))
        .distance(36.0)
        .size(1.0)
        .origin(DotsOrigin::Center);
    dots.update(AnimatorState::Entered);
    dots.paint(bounds, &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // Start frame animation
    if !state.frame_started {
        state.frame_started = true;
    }
    let frame_progress = state.frame_animator.update(AnimatorState::Entering);

    // Layout
    let padding = 24.0;
    let max_width = 900.0;
    let max_height = 700.0;
    let card_width = (width - padding * 2.0).min(max_width);
    let card_height = (height - padding * 2.0).min(max_height);
    let card_x = (width - card_width) / 2.0;
    let card_y = (height - card_height) / 2.0;
    let card_bounds = Bounds::new(card_x, card_y, card_width, card_height);

    // Main frame with corners
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    let mut frame = Frame::corners()
        .line_color(Hsla::new(0.0, 0.0, 1.0, 0.8))
        .bg_color(Hsla::new(0.0, 0.0, 0.0, 0.6))
        .glow_color(Hsla::new(0.0, 0.0, 1.0, 0.15))
        .border_color(Hsla::new(0.0, 0.0, 1.0, 0.1))
        .stroke_width(1.0)
        .corner_length(30.0)
        .animation_progress(frame_progress);
    frame.paint(card_bounds, &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // Inner content
    let inner_padding = 24.0;
    let inner_x = card_x + inner_padding;
    let inner_y = card_y + inner_padding;
    let inner_width = card_width - inner_padding * 2.0;
    let _inner_height = card_height - inner_padding * 2.0;

    // Title bar
    let title = "NEXUS RELAY";
    let title_size = 20.0;
    let title_run = text_system.layout(title, Point::new(inner_x, inner_y), title_size, theme::text::PRIMARY);
    scene.draw_text(title_run);

    // Connection status indicator
    let status_color = match state.connection_status {
        ConnectionStatus::Connected => Hsla::new(0.33, 0.8, 0.5, 1.0), // Green
        ConnectionStatus::Connecting => Hsla::new(0.15, 0.8, 0.5, 1.0), // Yellow
        ConnectionStatus::Disconnected => Hsla::new(0.0, 0.0, 0.4, 1.0), // Gray
        ConnectionStatus::Error => Hsla::new(0.0, 0.8, 0.5, 1.0), // Red
    };
    let status_text = match state.connection_status {
        ConnectionStatus::Connected => "CONNECTED",
        ConnectionStatus::Connecting => "CONNECTING",
        ConnectionStatus::Disconnected => "DISCONNECTED",
        ConnectionStatus::Error => "ERROR",
    };
    let status_size = 11.0;
    let status_width = text_system.measure(status_text, status_size);
    let status_x = card_x + card_width - inner_padding - status_width - 16.0;
    let status_run = text_system.layout(status_text, Point::new(status_x, inner_y + 4.0), status_size, status_color);
    scene.draw_text(status_run);

    // Status dot
    let dot_size = 8.0;
    let dot_x = status_x - dot_size - 8.0;
    let dot_y = inner_y + 8.0;
    scene.draw_quad(
        Quad::new(Bounds::new(dot_x, dot_y, dot_size, dot_size))
            .with_background(status_color)
            .with_corner_radius(dot_size / 2.0),
    );

    let mut y = inner_y + title_size + 24.0;

    // Divider line
    scene.draw_quad(
        Quad::new(Bounds::new(inner_x, y, inner_width, 1.0))
            .with_background(Hsla::new(0.0, 0.0, 1.0, 0.1)),
    );
    y += 16.0;

    // Two-column layout
    let col_gap = 24.0;
    let col_width = (inner_width - col_gap) / 2.0;
    let col1_x = inner_x;
    let col2_x = inner_x + col_width + col_gap;

    // Left column: Events
    render_section(scene, text_system, col1_x, y, col_width, "EVENTS", &[
        ("Total", format_number(state.stats.events.total)),
        ("24h", format_number(state.stats.events.last_24h)),
    ]);

    // Events by kind
    let mut kind_y = y + 70.0;
    for kc in state.stats.events.by_kind.iter().take(5) {
        let kind_label = format!("Kind {}", kc.kind);
        let kind_value = format_number(kc.count);
        render_stat_row(scene, text_system, col1_x, kind_y, col_width, &kind_label, &kind_value, theme::text::MUTED);
        kind_y += 18.0;
    }

    // Right column: Job Marketplace
    render_section(scene, text_system, col2_x, y, col_width, "JOB MARKETPLACE", &[
        ("Pending", format_number(state.stats.jobs.pending)),
        ("Completed 24h", format_number(state.stats.jobs.completed_24h)),
    ]);

    // Jobs by kind
    let mut job_y = y + 70.0;
    for kc in state.stats.jobs.by_kind.iter().take(3) {
        let kind_label = format!("Kind {}", kc.kind);
        let kind_value = format_number(kc.count);
        render_stat_row(scene, text_system, col2_x, job_y, col_width, &kind_label, &kind_value, theme::text::MUTED);
        job_y += 18.0;
    }

    y += 180.0;

    // Bottom section: Swarm Compute (RLM)
    scene.draw_quad(
        Quad::new(Bounds::new(inner_x, y, inner_width, 1.0))
            .with_background(Hsla::new(0.0, 0.0, 1.0, 0.1)),
    );
    y += 16.0;

    let swarm_label = "SWARM COMPUTE";
    let swarm_run = text_system.layout(swarm_label, Point::new(inner_x, y), 12.0, theme::text::PRIMARY);
    scene.draw_text(swarm_run);
    y += 24.0;

    // Providers Online (orange accent)
    render_stat_row(
        scene,
        text_system,
        inner_x,
        y,
        inner_width,
        "Providers Online",
        &format!("{}", state.stats.rlm.providers_active),
        Hsla::new(0.09, 0.9, 0.55, 1.0), // Orange
    );
    y += 22.0;

    // RLM Queries (24h)
    render_stat_row(
        scene,
        text_system,
        inner_x,
        y,
        inner_width,
        "RLM Queries (24h)",
        &format!("{}", state.stats.rlm.subqueries_24h),
        theme::text::PRIMARY,
    );
    y += 22.0;

    // Success Rate (capped at 100% since multiple providers can respond to same query)
    let success_rate = if state.stats.rlm.subqueries_total > 0 {
        let rate = (state.stats.rlm.results_total as f32 / state.stats.rlm.subqueries_total as f32 * 100.0) as u64;
        rate.min(100)
    } else {
        0
    };
    render_stat_row(
        scene,
        text_system,
        inner_x,
        y,
        inner_width,
        "Success Rate",
        &format!("{}%", success_rate),
        theme::text::PRIMARY,
    );
    y += 28.0;

    // Progress bar showing completed vs total (capped at 100%)
    let bar_height = 16.0;
    let bar_width = inner_width;
    let total = state.stats.rlm.subqueries_24h.max(1);
    let fill = (state.stats.rlm.results_24h as f32 / total as f32).min(1.0);

    // Background bar
    scene.draw_quad(
        Quad::new(Bounds::new(inner_x, y, bar_width, bar_height))
            .with_background(Hsla::new(0.0, 0.0, 1.0, 0.1)),
    );

    // Filled bar (orange)
    scene.draw_quad(
        Quad::new(Bounds::new(inner_x, y, bar_width * fill.min(1.0), bar_height))
            .with_background(Hsla::new(0.09, 0.8, 0.55, 0.9)),
    );

    // Label on bar
    let bar_label = format!(
        "{}/{} completed",
        state.stats.rlm.results_24h, state.stats.rlm.subqueries_24h
    );
    let bar_label_run = text_system.layout(&bar_label, Point::new(inner_x + 4.0, y + 2.0), 10.0, Hsla::new(0.0, 0.0, 0.0, 0.9));
    scene.draw_text(bar_label_run);

    y += bar_height + 8.0;

    // Footer with timestamp
    let footer_y = card_y + card_height - inner_padding - 12.0;
    if state.stats.timestamp > 0 {
        let ts_text = format!("Last update: {} fetches", state.fetch_count);
        let ts_run = text_system.layout(&ts_text, Point::new(inner_x, footer_y), 9.0, Hsla::new(0.0, 0.0, 0.4, 1.0));
        scene.draw_text(ts_run);
    }

    // Error message if any
    if let Some(ref err) = state.fetch_error {
        let err_run = text_system.layout(err, Point::new(inner_x + 150.0, footer_y), 9.0, Hsla::new(0.0, 0.8, 0.5, 1.0));
        scene.draw_text(err_run);
    }
}

/// Render a section with title and stats
fn render_section(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    y: f32,
    _width: f32,
    title: &str,
    stats: &[(&str, String)],
) {
    let title_run = text_system.layout(title, Point::new(x, y), 12.0, theme::text::PRIMARY);
    scene.draw_text(title_run);

    let mut stat_y = y + 20.0;
    for (label, value) in stats {
        render_stat_row(scene, text_system, x, stat_y, _width, label, value, theme::text::PRIMARY);
        stat_y += 22.0;
    }
}

/// Render a stat row with label and value
fn render_stat_row(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    y: f32,
    _width: f32,
    label: &str,
    value: &str,
    color: Hsla,
) {
    let label_run = text_system.layout(label, Point::new(x, y), 11.0, theme::text::MUTED);
    scene.draw_text(label_run);

    let value_x = x + 120.0;
    let value_run = text_system.layout(value, Point::new(value_x, y), 11.0, color);
    scene.draw_text(value_run);
}

/// Format large numbers with commas
fn format_number(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
