use wgpui::{Bounds, PaintContext, Quad, Text, theme};

use crate::constants::PANEL_PADDING;
use crate::state::Storybook;

impl Storybook {
    pub(crate) fn paint_overview(&self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let mut y = bounds.origin.y + PANEL_PADDING;
        let mut title = Text::new("Overview")
            .font_size(theme::font_size::BASE)
            .color(theme::text::PRIMARY);
        title.paint(
            Bounds::new(
                bounds.origin.x + PANEL_PADDING,
                y,
                bounds.size.width - PANEL_PADDING * 2.0,
                24.0,
            ),
            cx,
        );
        y += 32.0;

        let body = [
            "This gallery covers ACP components plus Arwes parity components.",
            "Use the navigation to explore each layer and Arwes variants.",
            "Scroll inside the content pane to see full permutations.",
        ];

        for line in body {
            let mut text = Text::new(line)
                .font_size(theme::font_size::SM)
                .color(theme::text::MUTED);
            text.paint(
                Bounds::new(
                    bounds.origin.x + PANEL_PADDING,
                    y,
                    bounds.size.width - PANEL_PADDING * 2.0,
                    20.0,
                ),
                cx,
            );
            y += 22.0;
        }
    }
}
