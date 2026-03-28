#![allow(clippy::all, clippy::pedantic, unfulfilled_lint_expectations)]
#![expect(
    clippy::expect_used,
    reason = "Example/demo lane accepts explicit fail-fast setup paths."
)]
#![expect(
    clippy::unwrap_used,
    reason = "Example/demo lane accepts explicit fail-fast setup paths."
)]
#![expect(
    clippy::panic,
    reason = "Example/demo lane accepts explicit fail-fast setup paths."
)]

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;

use wgpui::renderer::Renderer;
use wgpui::viz::badge::{BadgeTone, tone_color as badge_color};
use wgpui::viz::chart::{HistoryChartSeries, paint_history_chart_body};
use wgpui::viz::feed::{EventFeedRow, paint_event_feed_body};
use wgpui::viz::panel;
use wgpui::viz::provenance::{ProvenanceTone, tone_color as provenance_color};
use wgpui::viz::theme as viz_theme;
use wgpui::viz::topology::{TopologyNodeState, node_state_color};
use wgpui::{Bounds, PaintContext, Point, Quad, Scene, Size, TextSystem, theme};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}

#[derive(Default)]
struct App {
    state: Option<RenderState>,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    started_at: Instant,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("wgpui Viz Primitives")
                        .with_inner_size(winit::dpi::LogicalSize::new(1220, 860)),
                )
                .expect("Failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });
            let surface = instance
                .create_surface(window.clone())
                .expect("Failed to create surface");
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find adapter");
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Failed to create device");

            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let renderer = Renderer::new(&device, surface_format);

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system: TextSystem::new(1.0),
                started_at: Instant::now(),
            }
        });

        self.state = Some(state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.config.width = size.width.max(1);
                state.config.height = size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;
                let mut scene = Scene::new();
                build_demo(
                    &mut scene,
                    &mut state.text_system,
                    width,
                    height,
                    state.started_at.elapsed().as_secs_f32(),
                );

                let output = state
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("viz_primitives_encoder"),
                        });

                state
                    .renderer
                    .resize(&state.queue, Size::new(width, height), 1.0);
                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }
                state.renderer.prepare(
                    &state.device,
                    &state.queue,
                    &scene,
                    state.window.scale_factor() as f32,
                );
                state.renderer.render(&mut encoder, &view);
                state.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

fn build_demo(scene: &mut Scene, text_system: &mut TextSystem, width: f32, height: f32, time: f32) {
    let mut cx = PaintContext::new(scene, text_system, 1.0);
    let root = Bounds::new(0.0, 0.0, width, height);
    cx.scene
        .draw_quad(Quad::new(root).with_background(theme::bg::APP));

    let left = Bounds::new(24.0, 24.0, width * 0.32 - 30.0, 236.0);
    let right = Bounds::new(
        left.max_x() + 18.0,
        24.0,
        width - left.max_x() - 42.0,
        406.0,
    );
    let bottom = Bounds::new(
        24.0,
        right.max_y() + 18.0,
        width - 48.0,
        height - right.max_y() - 42.0,
    );
    let phase = (time * 0.18).fract();

    panel::paint_shell(left, viz_theme::track::PGOLF, &mut cx);
    panel::paint_title(
        left,
        "TRAINING VIZ TOKENS",
        viz_theme::track::PGOLF,
        &mut cx,
    );
    panel::paint_texture(left, viz_theme::track::PGOLF, phase, &mut cx);
    paint_token_badges(left, &mut cx);

    panel::paint_shell(right, viz_theme::series::LOSS, &mut cx);
    panel::paint_title(right, "SCALAR CHART", viz_theme::series::LOSS, &mut cx);
    paint_history_chart_body(
        right,
        viz_theme::series::LOSS,
        phase,
        Some("pgolf.run.11l // loss, ema, selectivity"),
        Some("sampled at shared plot-column density"),
        "No scalar history available.",
        &[
            HistoryChartSeries {
                label: "loss",
                values: &[2.8, 2.4, 2.1, 1.8, 1.56, 1.33, 1.21, 1.12, 1.04, 0.98],
                color: viz_theme::series::LOSS,
                fill_alpha: 0.18,
                line_alpha: 0.74,
            },
            HistoryChartSeries {
                label: "ema",
                values: &[2.7, 2.48, 2.26, 1.98, 1.72, 1.49, 1.32, 1.2, 1.11, 1.04],
                color: viz_theme::series::PROVENANCE,
                fill_alpha: 0.12,
                line_alpha: 0.86,
            },
            HistoryChartSeries {
                label: "selectivity",
                values: &[0.22, 0.24, 0.31, 0.36, 0.4, 0.47, 0.51, 0.56, 0.61, 0.66],
                color: viz_theme::series::HARDWARE,
                fill_alpha: 0.0,
                line_alpha: 0.92,
            },
        ],
        &mut cx,
    );

    panel::paint_shell(bottom, viz_theme::series::EVENTS, &mut cx);
    panel::paint_title(bottom, "EVENT RAIL", viz_theme::series::EVENTS, &mut cx);
    paint_event_feed_body(
        bottom,
        viz_theme::series::EVENTS,
        phase,
        "No events recorded.",
        &[
            EventFeedRow {
                label: Cow::Borrowed("score_closeout"),
                detail: Cow::Borrowed(
                    "Detached closeout receipt retained and linked into retained evidence.",
                ),
                color: provenance_color(ProvenanceTone::Evidence),
            },
            EventFeedRow {
                label: Cow::Borrowed("promotion_gate"),
                detail: Cow::Borrowed(
                    "Topology verdict remained warning while cluster drift stayed above threshold.",
                ),
                color: node_state_color(TopologyNodeState::Warning),
            },
            EventFeedRow {
                label: Cow::Borrowed("cache_refresh"),
                detail: Cow::Borrowed(
                    "Viewer fell back to cached bundle while live heartbeat aged past the freshness target.",
                ),
                color: provenance_color(ProvenanceTone::Cached),
            },
            EventFeedRow {
                label: Cow::Borrowed("lane_ready"),
                detail: Cow::Borrowed(
                    "Bounded XTRAIN handoff produced a comparable train_to_infer proof surface.",
                ),
                color: badge_color(BadgeTone::TrackXtrain),
            },
        ],
        &mut cx,
    );
}

fn paint_token_badges(bounds: Bounds, cx: &mut PaintContext) {
    let intro = [
        "Shared tokens live in wgpui::viz::theme.",
        "Charts bind to series tokens.",
        "Badges bind to state and track tokens.",
        "Panels bind to surface tokens.",
    ];
    let mut y = bounds.origin.y + 40.0;
    for line in intro {
        cx.scene.draw_text(cx.text.layout(
            line,
            Point::new(bounds.origin.x + 16.0, y),
            11.0,
            theme::text::PRIMARY,
        ));
        y += 18.0;
    }

    let badges = [
        ("PGOLF", badge_color(BadgeTone::TrackPgolf)),
        ("HOMEGOLF", badge_color(BadgeTone::TrackHomegolf)),
        ("XTRAIN", badge_color(BadgeTone::TrackXtrain)),
        ("LIVE", badge_color(BadgeTone::Live)),
        ("WARNING", badge_color(BadgeTone::Warning)),
        ("ERROR", badge_color(BadgeTone::Error)),
    ];

    let mut x = bounds.origin.x + 16.0;
    let badge_y = bounds.origin.y + 130.0;
    for (label, color) in badges {
        draw_badge(Bounds::new(x, badge_y, 94.0, 26.0), label, color, cx);
        x += 102.0;
        if x + 94.0 > bounds.max_x() - 12.0 {
            x = bounds.origin.x + 16.0;
        }
    }
}

fn draw_badge(bounds: Bounds, label: &str, color: wgpui::Hsla, cx: &mut PaintContext) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(color.with_alpha(0.12))
            .with_border(color.with_alpha(0.42), 1.0)
            .with_corner_radius(6.0),
    );
    cx.scene.draw_text(cx.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 7.0),
        10.0,
        color.with_alpha(0.94),
    ));
}
