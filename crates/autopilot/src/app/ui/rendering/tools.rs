fn render_tools(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    chat_layout: &ChatLayout,
    viewport_top: f32,
    viewport_bottom: f32,
    scale_factor: f32,
) {
    let content_x = chat_layout.content_x;
    let available_width = chat_layout.available_width;

    // Render inline tools (scrolls with messages, no panel background)
    {
        let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
        for inline_layout in &chat_layout.inline_tools {
            for block in &inline_layout.blocks {
                // Check if the tool block is visible in the viewport
                let block_top = block.card_bounds.origin.y;
                let block_bottom = block
                    .detail_bounds
                    .as_ref()
                    .map(|db| db.origin.y + db.size.height)
                    .unwrap_or(block.card_bounds.origin.y + block.card_bounds.size.height);

                if block_bottom > viewport_top && block_top < viewport_bottom {
                    if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                        tool.card.paint(block.card_bounds, &mut paint_cx);
                        if tool.status == ToolStatus::Running {
                            let ratio = tool
                                .elapsed_secs
                                .map(|elapsed| (elapsed / 6.0).min(1.0).max(0.1) as f32)
                                .unwrap_or(0.2_f32);
                            let bar_height = 2.0;
                            let bar_bounds = Bounds::new(
                                block.card_bounds.origin.x,
                                block.card_bounds.origin.y
                                    + block.card_bounds.size.height
                                    - bar_height,
                                block.card_bounds.size.width,
                                bar_height,
                            );
                            paint_cx.scene.draw_quad(
                                Quad::new(bar_bounds).with_background(palette.tool_progress_bg),
                            );
                            paint_cx.scene.draw_quad(
                                Quad::new(Bounds::new(
                                    bar_bounds.origin.x,
                                    bar_bounds.origin.y,
                                    bar_bounds.size.width * ratio,
                                    bar_bounds.size.height,
                                ))
                                .with_background(palette.tool_progress_fg),
                            );
                        }
                        if let Some(detail_bounds) = block.detail_bounds {
                            tool.detail.paint(detail_bounds, &mut paint_cx);
                        }
                    }
                }
            }
        }
    }

    // Render DSPy stage cards on layer 1 (above streaming text)
    {
        scene.set_layer(1);
        let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
        for dspy_layout in &chat_layout.dspy_stages {
            let stage_top = dspy_layout.y_offset;
            let stage_bottom = stage_top + dspy_layout.height;

            // Check if the stage is visible in the viewport
            if stage_bottom > viewport_top && stage_top < viewport_bottom {
                if let Some(stage_viz) = state.tools.dspy_stages.get(dspy_layout.stage_index) {
                    render_dspy_stage_card(
                        &stage_viz.stage,
                        Bounds::new(content_x, stage_top, available_width, dspy_layout.height),
                        &mut paint_cx,
                        palette,
                    );
                }
            }
        }
    }
}

