fn render_overlays(
    state: &mut AppState,
    scene: &mut Scene,
    bounds: Bounds,
    scale_factor: f32,
    palette: &UiPalette,
) {
    // Kitchen sink storybook overlay (covers full screen)
    if state.show_kitchen_sink {
        // Render on layer 1 to be on top of all layer 0 content
        scene.set_layer(1);

        paint_kitchen_sink(
            bounds,
            scene,
            &mut state.text_system,
            scale_factor,
            state.kitchen_sink_scroll,
            palette,
        );
    }

    if state.command_palette.is_open() {
        let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
        state.command_palette.paint(bounds, &mut paint_cx);
    }

    if state.chat.chat_context_menu.is_open() {
        scene.set_layer(1);
        let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
        state.chat.chat_context_menu.paint(bounds, &mut paint_cx);
    }

    if let Some(dialog) = state.permissions.permission_dialog.as_mut() {
        if dialog.is_open() {
            let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
            dialog.paint(bounds, &mut paint_cx);
        }
    }
}

