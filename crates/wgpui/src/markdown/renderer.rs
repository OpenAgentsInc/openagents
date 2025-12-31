use crate::geometry::{Bounds, Point, Size};
use crate::scene::{Quad, Scene};
use crate::text::{FontStyle, TextSystem};
use crate::{Hsla, theme};

use super::types::*;

pub struct MarkdownRenderer {
    config: MarkdownConfig,
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self::with_config(MarkdownConfig::default())
    }

    pub fn with_config(config: MarkdownConfig) -> Self {
        Self { config }
    }

    /// Measure the height that rendering this document would take, without actually drawing.
    pub fn measure(
        &self,
        document: &MarkdownDocument,
        max_width: f32,
        text_system: &mut TextSystem,
    ) -> Size {
        let mut y = 0.0;

        for (i, block) in document.blocks.iter().enumerate() {
            if i > 0 {
                y += theme::spacing::MD;
            }
            y += self.measure_block(block, max_width, text_system);
        }

        Size::new(max_width, y)
    }

    fn measure_block(
        &self,
        block: &MarkdownBlock,
        max_width: f32,
        text_system: &mut TextSystem,
    ) -> f32 {
        match block {
            MarkdownBlock::Paragraph(lines) => self.measure_lines(lines),

            MarkdownBlock::Header { level, lines } => {
                let margin_top = match level {
                    1 => theme::spacing::XL,
                    2 => theme::spacing::LG,
                    _ => theme::spacing::MD,
                };
                margin_top + self.measure_lines(lines)
            }

            MarkdownBlock::CodeBlock { lines, .. } => {
                let padding = theme::spacing::MD;
                let margin = theme::spacing::SM;
                let content_height: f32 = lines
                    .iter()
                    .map(|l| {
                        l.spans
                            .first()
                            .map(|s| s.style.font_size * l.line_height)
                            .unwrap_or(self.config.base_font_size * theme::line_height::NORMAL)
                    })
                    .sum();
                content_height + padding * 2.0 + margin * 2.0
            }

            MarkdownBlock::Blockquote(blocks) => {
                let bar_width = 4.0;
                let gap = theme::spacing::MD;
                let indent = bar_width + gap;
                let margin = theme::spacing::SM;
                let mut height = 0.0;
                for block in blocks {
                    height += self.measure_block(block, max_width - indent, text_system);
                }
                height + margin * 2.0
            }

            MarkdownBlock::UnorderedList(items) => {
                let indent = theme::spacing::XL;
                let margin = theme::spacing::XS;
                let mut height = margin;
                for item in items {
                    for block in item {
                        height += self.measure_block(block, max_width - indent, text_system);
                    }
                }
                height + margin
            }

            MarkdownBlock::OrderedList { items, .. } => {
                let indent = theme::spacing::XL * 2.0;
                let margin = theme::spacing::XS;
                let mut height = margin;
                for item in items {
                    for block in item {
                        height += self.measure_block(block, max_width - indent, text_system);
                    }
                }
                height + margin
            }

            MarkdownBlock::HorizontalRule => {
                let margin = theme::spacing::LG;
                margin * 2.0 + 1.0
            }

            MarkdownBlock::Table { headers, rows, .. } => {
                let header_height = 32.0;
                let row_height = 28.0;
                let separator = 1.0;
                if headers.is_empty() {
                    0.0
                } else {
                    header_height + separator + (rows.len() as f32 * row_height)
                }
            }
        }
    }

    fn measure_lines(&self, lines: &[StyledLine]) -> f32 {
        let mut height = 0.0;
        for line in lines {
            height += line.margin_top;
            let base_font_size = line
                .spans
                .first()
                .map(|s| s.style.font_size)
                .unwrap_or(self.config.base_font_size);
            height += base_font_size * line.line_height;
        }
        height
    }

    pub fn render(
        &self,
        document: &MarkdownDocument,
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
    ) -> Size {
        self.render_with_opacity(document, origin, max_width, text_system, scene, 1.0)
    }

    pub fn render_with_opacity(
        &self,
        document: &MarkdownDocument,
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
        opacity: f32,
    ) -> Size {
        let mut y = origin.y;
        let x = origin.x;

        for (i, block) in document.blocks.iter().enumerate() {
            if i > 0 {
                // Add generous spacing between blocks
                y += theme::spacing::MD;
            }

            y += self.render_block_with_opacity(
                block,
                Point::new(x, y),
                max_width,
                text_system,
                scene,
                opacity,
            );
        }

        Size::new(max_width, y - origin.y)
    }

    fn render_block_with_opacity(
        &self,
        block: &MarkdownBlock,
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
        opacity: f32,
    ) -> f32 {
        match block {
            MarkdownBlock::Paragraph(lines) => {
                self.render_lines(lines, origin, max_width, 0, text_system, scene, opacity)
            }

            MarkdownBlock::Header { level, lines } => {
                let margin_top = match level {
                    1 => theme::spacing::XL,
                    2 => theme::spacing::LG,
                    _ => theme::spacing::MD,
                };
                margin_top
                    + self.render_lines(
                        lines,
                        Point::new(origin.x, origin.y + margin_top),
                        max_width,
                        0,
                        text_system,
                        scene,
                        opacity,
                    )
            }

            MarkdownBlock::CodeBlock { lines, .. } => {
                self.render_code_block(lines, origin, max_width, text_system, scene, opacity)
            }

            MarkdownBlock::Blockquote(blocks) => {
                self.render_blockquote(blocks, origin, max_width, text_system, scene, opacity)
            }

            MarkdownBlock::UnorderedList(items) => {
                self.render_unordered_list(items, origin, max_width, text_system, scene, opacity)
            }

            MarkdownBlock::OrderedList { start, items } => self.render_ordered_list(
                *start,
                items,
                origin,
                max_width,
                text_system,
                scene,
                opacity,
            ),

            MarkdownBlock::HorizontalRule => {
                self.render_horizontal_rule(origin, max_width, scene, opacity)
            }

            MarkdownBlock::Table { headers, rows } => self.render_table(
                headers,
                rows,
                origin,
                max_width,
                text_system,
                scene,
                opacity,
            ),
        }
    }

    fn render_lines(
        &self,
        lines: &[StyledLine],
        origin: Point,
        _max_width: f32,
        base_indent: u32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
        opacity: f32,
    ) -> f32 {
        let mut y = origin.y;

        for line in lines {
            y += line.margin_top;
            let indent = (base_indent + line.indent) as f32 * theme::spacing::LG;
            let x = origin.x + indent;

            let mut span_x = x;
            // Ensure minimum line height based on font size
            let base_font_size = line
                .spans
                .first()
                .map(|s| s.style.font_size)
                .unwrap_or(self.config.base_font_size);
            let mut line_height = base_font_size * line.line_height;

            for span in &line.spans {
                let text_size = text_system.measure_size(&span.text, span.style.font_size, None);

                if let Some(bg) = span.style.background {
                    let padding = 2.0;
                    let bg_with_opacity = bg.with_alpha(bg.a * opacity);
                    scene.draw_quad(Quad {
                        bounds: Bounds::new(
                            span_x - padding,
                            y - padding,
                            text_size.width + padding * 2.0,
                            text_size.height + padding * 2.0,
                        ),
                        background: Some(bg_with_opacity),
                        border_color: Hsla::transparent(),
                        border_width: 0.0,
                        corner_radius: 0.0,
                    });
                }

                let color = span.style.color.with_alpha(span.style.color.a * opacity);

                let font_style = FontStyle {
                    bold: span.style.bold,
                    italic: span.style.italic,
                };

                let text_run = text_system.layout_styled(
                    &span.text,
                    Point::new(span_x, y),
                    span.style.font_size,
                    color,
                    font_style,
                );

                // CRITICAL: Use measure_styled with the SAME font_style as layout_styled!
                // Bold/italic text has different widths than regular text.
                let span_width =
                    text_system.measure_styled(&span.text, span.style.font_size, font_style);
                line_height = line_height.max(span.style.font_size * line.line_height);

                scene.draw_text(text_run);

                if span.style.strikethrough {
                    let strike_y = y + text_size.height * 0.5;
                    scene.draw_quad(Quad {
                        bounds: Bounds::new(span_x, strike_y, span_width, 1.0),
                        background: Some(color),
                        border_color: Hsla::transparent(),
                        border_width: 0.0,
                        corner_radius: 0.0,
                    });
                }

                span_x += span_width;
            }

            y += line_height;
        }

        y - origin.y
    }

    fn render_code_block(
        &self,
        lines: &[StyledLine],
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
        opacity: f32,
    ) -> f32 {
        let padding = theme::spacing::MD;
        let margin = theme::spacing::SM;

        let content_height: f32 = lines
            .iter()
            .map(|l| {
                l.spans
                    .first()
                    .map(|s| s.style.font_size * l.line_height)
                    .unwrap_or(self.config.base_font_size * theme::line_height::NORMAL)
            })
            .sum();

        let total_height = content_height + padding * 2.0;

        let bg_color = self
            .config
            .code_background
            .with_alpha(self.config.code_background.a * opacity);
        scene.draw_quad(Quad {
            bounds: Bounds::new(origin.x, origin.y + margin, max_width, total_height),
            background: Some(bg_color),
            border_color: Hsla::transparent(),
            border_width: 0.0,
            corner_radius: 0.0,
        });

        self.render_lines(
            lines,
            Point::new(origin.x + padding, origin.y + margin + padding),
            max_width - padding * 2.0,
            0,
            text_system,
            scene,
            opacity,
        );

        total_height + margin * 2.0
    }

    fn render_blockquote(
        &self,
        blocks: &[MarkdownBlock],
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
        opacity: f32,
    ) -> f32 {
        let bar_width = 4.0;
        let gap = theme::spacing::MD; // Gap between bar and text
        let indent = bar_width + gap;
        let margin = theme::spacing::SM;
        // Text ascent offset - text baseline is at Y, but visual top is higher
        // For ~14px font, ascent is about 11px
        let text_ascent = 11.0;

        let start_y = origin.y + margin;
        let mut y = start_y;

        for block in blocks {
            y += self.render_block_with_opacity(
                block,
                Point::new(origin.x + indent, y),
                max_width - indent,
                text_system,
                scene,
                opacity,
            );
        }

        let content_height = y - start_y;

        // Draw bar aligned with text visual top (accounting for text ascent)
        let bar_color = theme::accent::PRIMARY.with_alpha(opacity);
        let bar_top = start_y - text_ascent + 2.0; // Slight adjustment for visual centering
        scene.draw_quad(Quad {
            bounds: Bounds::new(origin.x, bar_top, bar_width, content_height + text_ascent),
            background: Some(bar_color),
            border_color: Hsla::transparent(),
            border_width: 0.0,
            corner_radius: 0.0,
        });

        content_height + margin * 2.0
    }

    fn render_unordered_list(
        &self,
        items: &[Vec<MarkdownBlock>],
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
        opacity: f32,
    ) -> f32 {
        let indent = theme::spacing::XL;
        let bullet_x = origin.x + theme::spacing::SM;
        let margin = theme::spacing::XS;

        let mut y = origin.y + margin;

        for item in items {
            let item_y = y;

            let bullet_color = self
                .config
                .text_color
                .with_alpha(self.config.text_color.a * opacity);
            let bullet_run = text_system.layout_styled(
                "\u{2022}",
                Point::new(bullet_x, item_y),
                self.config.base_font_size,
                bullet_color,
                FontStyle::normal(),
            );
            scene.draw_text(bullet_run);

            for block in item {
                y += self.render_block_with_opacity(
                    block,
                    Point::new(origin.x + indent, y),
                    max_width - indent,
                    text_system,
                    scene,
                    opacity,
                );
            }
        }

        y - origin.y + margin
    }

    fn render_ordered_list(
        &self,
        start: u64,
        items: &[Vec<MarkdownBlock>],
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
        opacity: f32,
    ) -> f32 {
        let indent = theme::spacing::XL * 2.0;
        let number_x = origin.x;
        let margin = theme::spacing::XS;

        let mut y = origin.y + margin;

        for (i, item) in items.iter().enumerate() {
            let item_y = y;
            let number = start + i as u64;

            let number_color = self
                .config
                .text_color
                .with_alpha(self.config.text_color.a * opacity);
            let number_run = text_system.layout_styled(
                &format!("{}.", number),
                Point::new(number_x, item_y),
                self.config.base_font_size,
                number_color,
                FontStyle::normal(),
            );
            scene.draw_text(number_run);

            for block in item {
                y += self.render_block_with_opacity(
                    block,
                    Point::new(origin.x + indent, y),
                    max_width - indent,
                    text_system,
                    scene,
                    opacity,
                );
            }
        }

        y - origin.y + margin
    }

    fn render_horizontal_rule(
        &self,
        origin: Point,
        max_width: f32,
        scene: &mut Scene,
        opacity: f32,
    ) -> f32 {
        let margin = theme::spacing::LG;

        let rule_color = theme::border::DEFAULT.with_alpha(opacity);
        scene.draw_quad(Quad {
            bounds: Bounds::new(origin.x, origin.y + margin, max_width, 1.0),
            background: Some(rule_color),
            border_color: Hsla::transparent(),
            border_width: 0.0,
            corner_radius: 0.0,
        });

        margin * 2.0 + 1.0
    }

    fn render_table(
        &self,
        headers: &[Vec<StyledLine>],
        rows: &[Vec<Vec<StyledLine>>],
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
        opacity: f32,
    ) -> f32 {
        if headers.is_empty() {
            return 0.0;
        }

        let cell_padding = theme::spacing::SM;
        let col_width = max_width / headers.len() as f32;
        let mut y = origin.y;

        let mut x = origin.x;
        for header in headers {
            self.render_lines(
                header,
                Point::new(x + cell_padding, y + cell_padding),
                col_width - cell_padding * 2.0,
                0,
                text_system,
                scene,
                opacity,
            );
            x += col_width;
        }
        y += 32.0;

        let border_color = theme::border::DEFAULT.with_alpha(opacity);
        scene.draw_quad(Quad {
            bounds: Bounds::new(origin.x, y, max_width, 1.0),
            background: Some(border_color),
            border_color: Hsla::transparent(),
            border_width: 0.0,
            corner_radius: 0.0,
        });
        y += 1.0;

        for row in rows {
            let mut x = origin.x;
            for cell in row {
                self.render_lines(
                    cell,
                    Point::new(x + cell_padding, y + cell_padding),
                    col_width - cell_padding * 2.0,
                    0,
                    text_system,
                    scene,
                    opacity,
                );
                x += col_width;
            }
            y += 28.0;
        }

        y - origin.y
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render_markdown(
    markdown: &str,
    origin: Point,
    max_width: f32,
    text_system: &mut TextSystem,
    scene: &mut Scene,
) -> Size {
    use super::parser::MarkdownParser;

    let parser = MarkdownParser::new();
    let renderer = MarkdownRenderer::new();
    let document = parser.parse(markdown);
    renderer.render(&document, origin, max_width, text_system, scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_new() {
        let renderer = MarkdownRenderer::new();
        assert_eq!(renderer.config.base_font_size, theme::font_size::BASE);
    }

    #[test]
    fn test_renderer_with_config() {
        let config = MarkdownConfig {
            base_font_size: 20.0,
            ..Default::default()
        };
        let renderer = MarkdownRenderer::with_config(config);
        assert_eq!(renderer.config.base_font_size, 20.0);
    }
}
