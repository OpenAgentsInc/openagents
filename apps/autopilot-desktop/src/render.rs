use std::sync::Arc;

use anyhow::{Context, Result};
use nostr::load_or_create_identity;
use wgpui::components::hud::{DotShape, DotsGrid, DotsOrigin};
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, Easing, Hsla, PaintContext, Point, Quad, Scene, Size, TextSystem, theme,
};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::app_state::{
    DesktopPane, PaneKind, RenderState, WINDOW_HEIGHT, WINDOW_TITLE, WINDOW_WIDTH,
};
use crate::hotbar::{configure_hotbar, hotbar_bounds, new_hotbar};
use crate::pane_system::{
    PANE_MIN_HEIGHT, PANE_MIN_WIDTH, PANE_TITLE_HEIGHT, active_pane_id, create_empty_pane,
    nostr_regenerate_button_bounds, pane_content_bounds,
};
use crate::spark_pane;

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

        let async_runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to initialize async runtime")?;

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
            async_runtime,
            panes: Vec::new(),
            nostr_identity,
            nostr_identity_error,
            spark_wallet: crate::spark_wallet::SparkPaneState::default(),
            spark_inputs: crate::app_state::SparkPaneInputs::default(),
            next_pane_id: 1,
            next_z_index: 1,
            pane_drag_mode: None,
            pane_resizer: wgpui::components::hud::ResizablePane::new()
                .min_size(PANE_MIN_WIDTH, PANE_MIN_HEIGHT),
            hotbar_flash_was_active: false,
        };
        create_empty_pane(&mut state);
        Ok(state)
    })
}

pub fn render_frame(state: &mut RenderState) -> Result<()> {
    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let height = logical.height;
    let active_pane = active_pane_id(state);

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

        let hotbar_layer = paint_panes(
            &mut state.panes,
            active_pane,
            state.nostr_identity.as_ref(),
            state.nostr_identity_error.as_deref(),
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

fn paint_panes(
    panes: &mut [DesktopPane],
    active_id: Option<u64>,
    nostr_identity: Option<&nostr::NostrIdentity>,
    nostr_identity_error: Option<&str>,
    spark_wallet: &crate::spark_wallet::SparkPaneState,
    spark_inputs: &mut crate::app_state::SparkPaneInputs,
    paint: &mut PaintContext,
) -> u32 {
    let mut indices: Vec<usize> = (0..panes.len()).collect();
    indices.sort_by_key(|idx| panes[*idx].z_index);

    let mut next_layer: u32 = 1;
    for idx in indices {
        paint.scene.set_layer(next_layer);
        next_layer = next_layer.saturating_add(1);

        let pane = &mut panes[idx];

        paint
            .scene
            .draw_quad(Quad::new(pane.bounds).with_background(theme::bg::APP));

        pane.frame.set_title(&pane.title);
        pane.frame.set_active(active_id == Some(pane.id));
        pane.frame.set_title_height(PANE_TITLE_HEIGHT);
        pane.frame.paint(pane.bounds, paint);

        let content_bounds = pane_content_bounds(pane.bounds);
        paint.scene.draw_quad(
            Quad::new(content_bounds)
                .with_background(theme::bg::SURFACE)
                .with_corner_radius(0.0),
        );

        match pane.kind {
            PaneKind::Empty => paint_empty_pane(content_bounds, paint),
            PaneKind::NostrIdentity => {
                paint_nostr_identity_pane(
                    content_bounds,
                    nostr_identity,
                    nostr_identity_error,
                    paint,
                );
            }
            PaneKind::SparkWallet => {
                paint_spark_wallet_pane(content_bounds, spark_wallet, spark_inputs, paint);
            }
        }
    }

    next_layer
}

fn paint_empty_pane(content_bounds: Bounds, paint: &mut PaintContext) {
    let empty = paint.text.layout(
        "Empty pane",
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 16.0,
        ),
        12.0,
        theme::text::MUTED,
    );
    paint.scene.draw_text(empty);
}

fn paint_nostr_identity_pane(
    content_bounds: Bounds,
    nostr_identity: Option<&nostr::NostrIdentity>,
    nostr_identity_error: Option<&str>,
    paint: &mut PaintContext,
) {
    let regenerate_bounds = nostr_regenerate_button_bounds(content_bounds);
    paint.scene.draw_quad(
        Quad::new(regenerate_bounds)
            .with_background(theme::accent::PRIMARY.with_alpha(0.15))
            .with_border(theme::accent::PRIMARY, 1.0)
            .with_corner_radius(4.0),
    );
    paint.scene.draw_text(paint.text.layout(
        "Regenerate keys",
        Point::new(
            regenerate_bounds.origin.x + 10.0,
            regenerate_bounds.origin.y + 10.0,
        ),
        11.0,
        theme::text::PRIMARY,
    ));

    let mut y = regenerate_bounds.origin.y + regenerate_bounds.size.height + 14.0;
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Identity path",
        &nostr_identity.map_or_else(
            || "Unavailable".to_string(),
            |identity| identity.identity_path.display().to_string(),
        ),
    );

    if let Some(identity) = nostr_identity {
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "npub",
            &identity.npub,
        );
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "nsec",
            &identity.nsec,
        );
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Public key (hex)",
            &identity.public_key_hex,
        );
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Private key (hex)",
            &identity.private_key_hex,
        );
        let _ = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Mnemonic",
            &identity.mnemonic,
        );
    } else if let Some(error) = nostr_identity_error {
        let _ = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Identity error",
            error,
        );
    }
}

fn paint_spark_wallet_pane(
    content_bounds: Bounds,
    spark_wallet: &crate::spark_wallet::SparkPaneState,
    spark_inputs: &mut crate::app_state::SparkPaneInputs,
    paint: &mut PaintContext,
) {
    let layout = spark_pane::layout(content_bounds);

    paint_action_button(layout.refresh_button, "Refresh wallet", paint);
    paint_action_button(layout.spark_address_button, "Spark receive", paint);
    paint_action_button(layout.bitcoin_address_button, "Bitcoin receive", paint);
    paint_action_button(layout.create_invoice_button, "Create invoice", paint);
    paint_action_button(layout.send_payment_button, "Send payment", paint);

    spark_inputs
        .invoice_amount
        .set_max_width(layout.invoice_amount_input.size.width);
    spark_inputs
        .send_request
        .set_max_width(layout.send_request_input.size.width);
    spark_inputs
        .send_amount
        .set_max_width(layout.send_amount_input.size.width);

    spark_inputs
        .invoice_amount
        .paint(layout.invoice_amount_input, paint);
    spark_inputs
        .send_request
        .paint(layout.send_request_input, paint);
    spark_inputs
        .send_amount
        .paint(layout.send_amount_input, paint);

    paint.scene.draw_text(paint.text.layout(
        "Invoice sats",
        Point::new(
            layout.invoice_amount_input.origin.x,
            layout.invoice_amount_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Send request / invoice",
        Point::new(
            layout.send_request_input.origin.x,
            layout.send_request_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Send sats (optional)",
        Point::new(
            layout.send_amount_input.origin.x,
            layout.send_amount_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));

    let mut y = layout.details_origin.y;
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Network",
        spark_wallet.network_name(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Connection",
        spark_wallet.network_status_label(),
    );

    let (spark_sats, lightning_sats, onchain_sats, total_sats) =
        if let Some(balance) = spark_wallet.balance.as_ref() {
            (
                balance.spark_sats,
                balance.lightning_sats,
                balance.onchain_sats,
                balance.total_sats(),
            )
        } else {
            (0, 0, 0, 0)
        };
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Spark sats",
        &spark_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Lightning sats",
        &lightning_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Onchain sats",
        &onchain_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Total sats",
        &total_sats.to_string(),
    );

    if let Some(path) = spark_wallet.identity_path.as_ref() {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Identity path",
            &path.display().to_string(),
        );
    }
    if let Some(address) = spark_wallet.spark_address.as_deref() {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Spark address",
            address,
        );
    }
    if let Some(address) = spark_wallet.bitcoin_address.as_deref() {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Bitcoin address",
            address,
        );
    }
    if let Some(invoice) = spark_wallet.last_invoice.as_deref() {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last invoice",
            invoice,
        );
    }
    if let Some(payment_id) = spark_wallet.last_payment_id.as_deref() {
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last payment id",
            payment_id,
        );
    }
    if let Some(last_action) = spark_wallet.last_action.as_deref() {
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last action",
            last_action,
        );
    }
    if let Some(error) = spark_wallet.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            "Error:",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::status::ERROR,
        ));
        y += 16.0;
        for line in split_text_for_display(error, 88) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(content_bounds.origin.x + 12.0, y),
                11.0,
                theme::status::ERROR,
            ));
            y += 16.0;
        }
    }

    if !spark_wallet.recent_payments.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "Recent payments",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        y += 16.0;

        for payment in spark_wallet.recent_payments.iter().take(6) {
            let line = format!(
                "{} {} {} sats [{}]",
                payment.direction, payment.status, payment.amount_sats, payment.id
            );
            paint.scene.draw_text(paint.text.layout_mono(
                &line,
                Point::new(content_bounds.origin.x + 12.0, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
        }
    }
}

fn paint_action_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::accent::PRIMARY.with_alpha(0.15))
            .with_border(theme::accent::PRIMARY, 1.0)
            .with_corner_radius(4.0),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        11.0,
        theme::text::PRIMARY,
    ));
}

fn paint_label_line(paint: &mut PaintContext, x: f32, y: f32, label: &str, value: &str) -> f32 {
    paint.scene.draw_text(paint.text.layout(
        &format!("{label}:"),
        Point::new(x, y),
        11.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(x + 122.0, y),
        11.0,
        theme::text::PRIMARY,
    ));
    y + 16.0
}

fn paint_multiline_phrase(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
) -> f32 {
    paint.scene.draw_text(paint.text.layout(
        &format!("{label}:"),
        Point::new(x, y),
        11.0,
        theme::text::MUTED,
    ));

    let mut line_y = y;
    for chunk in split_text_for_display(value, 72) {
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(x + 122.0, line_y),
            11.0,
            theme::text::PRIMARY,
        ));
        line_y += 16.0;
    }
    line_y
}

fn split_text_for_display(text: &str, chunk_len: usize) -> Vec<String> {
    if text.trim().is_empty() {
        return vec![String::new()];
    }

    let chars: Vec<char> = text.chars().collect();
    chars
        .chunks(chunk_len.max(1))
        .map(|chunk| chunk.iter().collect())
        .collect()
}

pub fn logical_size(config: &wgpu::SurfaceConfiguration, scale_factor: f32) -> Size {
    let scale = scale_factor.max(0.1);
    Size::new(config.width as f32 / scale, config.height as f32 / scale)
}
