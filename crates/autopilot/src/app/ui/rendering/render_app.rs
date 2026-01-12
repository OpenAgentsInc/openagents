pub(crate) fn render_app(state: &mut AppState) {
    let scale_factor = state.window.scale_factor() as f32;
    let logical_width = state.config.width as f32 / scale_factor;
    let logical_height = state.config.height as f32 / scale_factor;

    let output = match state.surface.get_current_texture() {
        Ok(t) => t,
        Err(wgpu::SurfaceError::Lost) => {
            state.surface.configure(&state.device, &state.config);
            return;
        }
        Err(wgpu::SurfaceError::OutOfMemory) => {
            tracing::error!("Out of memory");
            return;
        }
        Err(_) => return,
    };
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut scene = Scene::new();
    let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);
    let palette = palette_for(state.settings.coder_settings.theme);
    let sidebar_layout = sidebar_layout(
        logical_width,
        logical_height,
        state.left_sidebar_open,
        state.right_sidebar_open,
    );

    // Dark terminal background
    scene.draw_quad(Quad::new(bounds).with_background(palette.background));

    render_sidebars(state, &mut scene, &palette, &sidebar_layout);
    render_topbar(state, &mut scene, &palette, &sidebar_layout);
    render_chat(
        state,
        &mut scene,
        &palette,
        &sidebar_layout,
        logical_height,
        scale_factor,
    );
    render_input(
        state,
        &mut scene,
        &palette,
        &sidebar_layout,
        logical_width,
        logical_height,
        scale_factor,
    );
    render_modals(
        state,
        &mut scene,
        &palette,
        &sidebar_layout,
        logical_width,
        logical_height,
        scale_factor,
    );
    render_overlays(state, &mut scene, bounds, scale_factor, &palette);

    // Render
    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Autopilot Render"),
        });

    let physical_width = state.config.width as f32;
    let physical_height = state.config.height as f32;

    state
        .renderer
        .resize(&state.queue, Size::new(physical_width, physical_height), 1.0);

    if state.text_system.is_dirty() {
        state.renderer.update_atlas(
            &state.queue,
            state.text_system.atlas_data(),
            state.text_system.atlas_size(),
        );
        state.text_system.mark_clean();
    }

    state
        .renderer
        .prepare(&state.device, &state.queue, &scene, scale_factor);
    state.renderer.render(&mut encoder, &view);

    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();
}
