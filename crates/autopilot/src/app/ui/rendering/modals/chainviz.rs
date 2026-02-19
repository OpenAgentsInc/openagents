use crate::app::chainviz::{CHAINVIZ_NODE_GAP, CHAINVIZ_PADDING};

fn render_chainviz_modal(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    bounds: Bounds,
    logical_width: f32,
    logical_height: f32,
    scale_factor: f32,
) {
            scene.set_layer(1);

            let overlay = Quad::new(bounds).with_background(palette.overlay);
            scene.draw_quad(overlay);

            let margin = (logical_width.min(logical_height) * 0.06).max(24.0);
            let modal_width = (logical_width - margin * 2.0).max(320.0);
            let modal_height = (logical_height - margin * 2.0).max(240.0);
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = (logical_height - modal_height) / 2.0;
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(palette.panel)
                .with_border(palette.panel_border, 1.0);
            scene.draw_quad(modal_bg);

            let title_y = modal_y + 12.0;
            let title_run = state.text_system.layout_styled_mono(
                "DSPy Chain Visualizer",
                Point::new(modal_x + CHAINVIZ_PADDING, title_y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);

            let header_height = 28.0;
            let footer_padding = 16.0;
            let content_top = modal_y + header_height + 8.0;
            let content_bottom = modal_y + modal_height - footer_padding;
            let content_width = modal_width - CHAINVIZ_PADDING * 2.0;
            let viewport_height = (content_bottom - content_top).max(0.0);

            if state.chainviz.drain_events() {
                state.window.request_redraw();
            }

            state.chainviz.viewport_height = viewport_height;

            let Some(chain_state) = state.chainviz.chain_state.as_ref() else {
                let empty_run = state.text_system.layout_styled_mono(
                    "No chain running. Use /chainviz <prompt> to start.",
                    Point::new(modal_x + CHAINVIZ_PADDING, content_top + 8.0),
                    12.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
                return;
            };

            let theme = crate::app::chainviz::components::ChainTheme::from_palette(palette);
            let chain_state = chain_state.lock().unwrap();

            let prompt_card =
                crate::app::chainviz::components::PromptCard::new(&chain_state.prompt);
            let prompt_height =
                prompt_card.height(content_width, &mut state.text_system, scale_factor);

            let mut total_height = CHAINVIZ_PADDING + prompt_height + CHAINVIZ_NODE_GAP;
            let nodes = chain_state.nodes();
            for node in nodes.iter() {
                let node_height = node.height(content_width, &mut state.text_system, scale_factor);
                total_height += node_height + CHAINVIZ_NODE_GAP;
            }
            total_height += CHAINVIZ_PADDING;
            state.chainviz.content_height = total_height;
            let max_scroll = (state.chainviz.content_height - state.chainviz.viewport_height)
                .max(0.0);
            if state.chainviz.scroll_offset > max_scroll {
                state.chainviz.scroll_offset = max_scroll;
            }

            let mut y = content_top + CHAINVIZ_PADDING - state.chainviz.scroll_offset;
            let content_left = modal_x + CHAINVIZ_PADDING;

            if y + prompt_height > content_top && y < content_bottom {
                prompt_card.paint(
                    Bounds::new(content_left, y, content_width, prompt_height),
                    scene,
                    &mut state.text_system,
                    &theme,
                    scale_factor,
                );
            }
            y += prompt_height + CHAINVIZ_NODE_GAP;

            for node in nodes.iter() {
                let node_height = node.height(content_width, &mut state.text_system, scale_factor);
                if y + node_height > content_top && y < content_bottom {
                    let connector_top = y - CHAINVIZ_NODE_GAP + 4.0;
                    let connector_bottom = y - 4.0;
                    if connector_bottom > content_top && connector_top < content_bottom {
                        crate::app::chainviz::components::Connector::paint(
                            connector_top,
                            connector_bottom,
                            modal_x + modal_width / 2.0,
                            scene,
                            &theme,
                        );
                    }
                    node.paint(
                        Bounds::new(content_left, y, content_width, node_height),
                        scene,
                        &mut state.text_system,
                        &theme,
                        scale_factor,
                    );
                } else if y > content_bottom {
                    break;
                }
                y += node_height + CHAINVIZ_NODE_GAP;
            }
}
