use std::sync::Arc;

use anyhow::{Context, Result};
use nostr::load_or_create_identity;
use wgpui::components::hud::{DotShape, DotsGrid, DotsOrigin};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Component, Easing, Hsla, PaintContext, Quad, Scene, Size, TextSystem, theme};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::app_state::{RenderState, WINDOW_HEIGHT, WINDOW_TITLE, WINDOW_WIDTH};
use crate::hotbar::{configure_hotbar, hotbar_bounds, new_hotbar};
use crate::pane_renderer::PaneRenderer;
use crate::pane_system::{PANE_MIN_HEIGHT, PANE_MIN_WIDTH, PaneController};

const GRID_DOT_DISTANCE: f32 = 32.0;

pub fn init_state(event_loop: &ActiveEventLoop) -> Result<RenderState> {
    let window_attrs = Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));

    let window = Arc::new(
        event_loop
            .create_window(window_attrs)
            .context("failed to create window")?,
    );

    pollster::block_on(async move {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .context("failed to create surface")?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("failed to find compatible adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .context("failed to create device")?;

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|format| format.is_srgb())
            .copied()
            .or_else(|| surface_caps.formats.first().copied())
            .context("surface formats empty")?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let renderer = Renderer::new(&device, surface_format);
        let scale_factor = window.scale_factor() as f32;
        let text_system = TextSystem::new(scale_factor);

        let hotbar = new_hotbar();
        let initial_hotbar_bounds = hotbar_bounds(logical_size(&config, scale_factor));

        let (nostr_identity, nostr_identity_error) = match load_or_create_identity() {
            Ok(identity) => (Some(identity), None),
            Err(err) => (None, Some(err.to_string())),
        };

        let spark_wallet = crate::spark_wallet::SparkPaneState::default();
        let spark_worker = crate::spark_wallet::SparkWalletWorker::spawn(spark_wallet.network);

        let mut state = RenderState {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            text_system,
            scale_factor,
            hotbar,
            hotbar_bounds: initial_hotbar_bounds,
            event_context: wgpui::EventContext::new(),
            input_modifiers: wgpui::Modifiers::default(),
            panes: Vec::new(),
            nostr_identity,
            nostr_identity_error,
            nostr_secret_state: crate::app_state::NostrSecretState::default(),
            spark_wallet,
            spark_worker,
            spark_inputs: crate::app_state::SparkPaneInputs::default(),
            next_pane_id: 1,
            next_z_index: 1,
            pane_drag_mode: None,
            pane_resizer: wgpui::components::hud::ResizablePane::new()
                .min_size(PANE_MIN_WIDTH, PANE_MIN_HEIGHT),
            hotbar_flash_was_active: false,
        };
        PaneController::create_empty(&mut state);
        Ok(state)
    })
}

pub fn render_frame(state: &mut RenderState) -> Result<()> {
    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let height = logical.height;
    let active_pane = PaneController::active(state);

    let mut scene = Scene::new();
    scene
        .draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    {
        let mut paint = PaintContext::new(&mut scene, &mut state.text_system, state.scale_factor);

        let mut dots_grid = DotsGrid::new()
            .color(Hsla::new(0.0, 0.0, 0.30, 0.26))
            .shape(DotShape::Cross)
            .distance(GRID_DOT_DISTANCE)
            .size(5.0)
            .cross_thickness(1.0)
            .origin(DotsOrigin::Center)
            .easing(Easing::EaseOut);
        dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut paint);

        let hotbar_layer = PaneRenderer::paint(
            &mut state.panes,
            active_pane,
            state.nostr_identity.as_ref(),
            state.nostr_identity_error.as_deref(),
            &state.nostr_secret_state,
            &state.spark_wallet,
            &mut state.spark_inputs,
            &mut paint,
        );
        paint.scene.set_layer(hotbar_layer);

        let bar_bounds = hotbar_bounds(logical);
        state.hotbar_bounds = bar_bounds;
        configure_hotbar(&mut state.hotbar);
        state.hotbar.paint(bar_bounds, &mut paint);
    }

    state
        .renderer
        .resize(&state.queue, logical, state.scale_factor.max(0.1));

    if state.text_system.is_dirty() {
        state.renderer.update_atlas(
            &state.queue,
            state.text_system.atlas_data(),
            state.text_system.atlas_size(),
        );
        state.text_system.mark_clean();
    }

    let output = match state.surface.get_current_texture() {
        Ok(frame) => frame,
        Err(wgpu::SurfaceError::Lost) => {
            state.surface.configure(&state.device, &state.config);
            return Ok(());
        }
        Err(err) => return Err(anyhow::anyhow!("surface error: {err:?}")),
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Autopilot Render Encoder"),
        });

    state.renderer.prepare(
        &state.device,
        &state.queue,
        &scene,
        state.scale_factor.max(0.1),
    );
    state.renderer.render(&mut encoder, &view);
    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
}

pub fn logical_size(config: &wgpu::SurfaceConfiguration, scale_factor: f32) -> Size {
    let scale = scale_factor.max(0.1);
    Size::new(config.width as f32 / scale, config.height as f32 / scale)
}
