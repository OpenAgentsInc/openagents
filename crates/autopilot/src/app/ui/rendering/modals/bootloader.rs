use crate::app::bootloader::BootConnector;

const BOOTLOADER_PADDING: f32 = 16.0;
const BOOTLOADER_NODE_GAP: f32 = 16.0;

fn render_bootloader_modal(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    bounds: Bounds,
    logical_width: f32,
    logical_height: f32,
    scale_factor: f32,
) {
    scene.set_layer(1);

    // Full-screen overlay (dimmed background)
    let overlay = Quad::new(bounds).with_background(palette.overlay);
    scene.draw_quad(overlay);

    // Modal dimensions - larger for bootloader
    let margin = (logical_width.min(logical_height) * 0.08).max(32.0);
    let modal_width = (logical_width - margin * 2.0).min(600.0).max(320.0);
    let modal_height = (logical_height - margin * 2.0).max(400.0);
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = (logical_height - modal_height) / 2.0;
    let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

    // Modal background
    let modal_bg = Quad::new(modal_bounds)
        .with_background(palette.panel)
        .with_border(palette.panel_border, 1.0);
    scene.draw_quad(modal_bg);

    // Title
    let title_y = modal_y + 16.0;
    let title_run = state.text_system.layout_styled_mono(
        "=== OpenAgents Boot ===",
        Point::new(modal_x + BOOTLOADER_PADDING, title_y),
        16.0,
        palette.link,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(title_run);

    // Drain events and request redraw if updated
    if state.bootloader.drain_events() {
        state.window.request_redraw();
    }

    // Content area
    let header_height = 40.0;
    let footer_padding = 16.0;
    let content_top = modal_y + header_height + 8.0;
    let content_bottom = modal_y + modal_height - footer_padding;
    let content_width = modal_width - BOOTLOADER_PADDING * 2.0;
    let viewport_height = (content_bottom - content_top).max(0.0);

    state.bootloader.viewport_height = viewport_height;

    // Calculate total content height
    let mut total_height = BOOTLOADER_PADDING;
    for card in &state.bootloader.cards {
        let card_height = card.height(content_width, &mut state.text_system, scale_factor);
        total_height += card_height + BOOTLOADER_NODE_GAP;
    }
    total_height += BOOTLOADER_PADDING;
    state.bootloader.content_height = total_height;

    // Clamp scroll offset
    let max_scroll = (state.bootloader.content_height - state.bootloader.viewport_height).max(0.0);
    if state.bootloader.scroll_offset > max_scroll {
        state.bootloader.scroll_offset = max_scroll;
    }

    // Render cards
    let mut y = content_top + BOOTLOADER_PADDING - state.bootloader.scroll_offset;
    let content_left = modal_x + BOOTLOADER_PADDING;

    for (idx, card) in state.bootloader.cards.iter().enumerate() {
        let card_height = card.height(content_width, &mut state.text_system, scale_factor);

        // Draw connector above card (except first)
        if idx > 0 && y > content_top {
            let connector_top = y - BOOTLOADER_NODE_GAP + 4.0;
            let connector_bottom = y - 4.0;
            if connector_bottom > content_top && connector_top < content_bottom {
                BootConnector::paint(
                    connector_top,
                    connector_bottom,
                    modal_x + modal_width / 2.0,
                    scene,
                    palette,
                );
            }
        }

        // Draw card if visible
        if y + card_height > content_top && y < content_bottom {
            card.paint(
                Bounds::new(content_left, y, content_width, card_height),
                scene,
                &mut state.text_system,
                palette,
                scale_factor,
            );
        } else if y > content_bottom {
            break;
        }

        y += card_height + BOOTLOADER_NODE_GAP;
    }

    // Summary footer if boot completed
    if let Some(summary) = &state.bootloader.summary {
        let footer_y = modal_y + modal_height - footer_padding - 16.0;

        // Separator
        scene.draw_quad(
            Quad::new(Bounds::new(
                modal_x + BOOTLOADER_PADDING,
                footer_y - 8.0,
                modal_width - BOOTLOADER_PADDING * 2.0,
                1.0,
            ))
            .with_background(palette.panel_border),
        );

        // Summary text
        for (i, line) in summary.lines().take(2).enumerate() {
            let summary_run = state.text_system.layout_mono(
                line,
                Point::new(modal_x + BOOTLOADER_PADDING, footer_y + (i as f32 * 14.0)),
                12.0,
                palette.link,
            );
            scene.draw_text(summary_run);
        }
    }

    // Error message if boot failed
    if let Some(error) = &state.bootloader.error_message {
        let error_y = modal_y + modal_height - footer_padding - 24.0;
        let error_run = state.text_system.layout_mono(
            &format!("Error: {}", error),
            Point::new(modal_x + BOOTLOADER_PADDING, error_y),
            12.0,
            wgpui::Hsla::new(0.0, 0.6, 0.5, 1.0), // Red
        );
        scene.draw_text(error_run);
    }
}
