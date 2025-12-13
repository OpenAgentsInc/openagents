//! Markdown renderer that outputs to Scene.

use crate::geometry::{Bounds, Point, Size};
use crate::scene::{Quad, Scene};
use crate::text::{FontStyle, TextSystem};
use crate::theme;

use super::types::*;

/// Renders markdown documents to a Scene.
pub struct MarkdownRenderer {
    config: MarkdownConfig,
}

impl MarkdownRenderer {
    /// Create a new renderer with default configuration.
    pub fn new() -> Self {
        Self::with_config(MarkdownConfig::default())
    }

    /// Create a new renderer with custom configuration.
    pub fn with_config(config: MarkdownConfig) -> Self {
        Self { config }
    }

    /// Render a markdown document to a scene.
    ///
    /// Returns the total size of the rendered content.
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

    /// Render a markdown document with a global opacity multiplier.
    ///
    /// Use this for fade-in effects during streaming.
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
            // Add spacing between blocks
            if i > 0 {
                y += theme::spacing::SM;
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

            MarkdownBlock::OrderedList { start, items } => {
                self.render_ordered_list(*start, items, origin, max_width, text_system, scene, opacity)
            }

            MarkdownBlock::HorizontalRule => {
                self.render_horizontal_rule(origin, max_width, scene, opacity)
            }

            MarkdownBlock::Table { headers, rows } => {
                self.render_table(headers, rows, origin, max_width, text_system, scene, opacity)
            }
        }
    }

    fn render_lines(
        &self,
        lines: &[StyledLine],
        origin: Point,
        max_width: f32,
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
            let _line_width = max_width - indent;

            // Render each span
            let mut span_x = x;
            let mut line_height = 0.0f32;

            for span in &line.spans {
                let text_size = text_system.measure_size(
                    &span.text,
                    span.style.font_size,
                    None,
                );

                // Render background if present (with opacity)
                if let Some(bg) = span.style.background {
                    let padding = 2.0;
                    let bg_with_opacity = bg.with_alpha(bg.a * opacity);
                    scene.draw_quad(
                        Quad::new(Bounds::new(
                            span_x - padding,
                            y - padding,
                            text_size.width + padding * 2.0,
                            text_size.height + padding * 2.0,
                        ))
                        .with_background(bg_with_opacity)
                        .with_uniform_radius(2.0),
                    );
                }

                // Use the span's color with opacity applied
                let color = span.style.color.with_alpha(span.style.color.a * opacity);

                // Build font style from span properties
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

                // Track line dimensions
                let span_width = text_system.measure(&span.text, span.style.font_size);
                line_height = line_height.max(span.style.font_size * line.line_height);

                scene.draw_text(text_run);

                // Draw strikethrough line if needed
                if span.style.strikethrough {
                    let strike_y = y + text_size.height * 0.4; // Middle of text
                    let strike_color = theme::accent::RED.with_alpha(opacity);
                    scene.draw_quad(
                        Quad::new(Bounds::new(span_x, strike_y, span_width, 2.0))
                            .with_background(strike_color),
                    );
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

        // Calculate content height
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

        // Draw background with opacity
        let bg_color = self.config.code_background.with_alpha(self.config.code_background.a * opacity);
        scene.draw_quad(
            Quad::new(Bounds::new(origin.x, origin.y + margin, max_width, total_height))
                .with_background(bg_color)
                .with_uniform_radius(theme::radius::DEFAULT),
        );

        // Draw lines
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
        let indent = theme::spacing::LG + bar_width;
        let margin = theme::spacing::SM;

        // Draw quote bar FIRST so it's behind content
        // We need to calculate height first by measuring content
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

        // Draw yellow quote bar on the left with opacity
        let bar_color = theme::accent::PRIMARY.with_alpha(opacity);
        scene.draw_quad(
            Quad::new(Bounds::new(
                origin.x,
                start_y,
                bar_width,
                content_height.max(20.0),
            ))
            .with_background(bar_color),
        );

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

            // Render bullet with opacity
            let bullet_color = self.config.text_color.with_alpha(self.config.text_color.a * opacity);
            let bullet_run = text_system.layout(
                "\u{2022}", // bullet character
                Point::new(bullet_x, item_y),
                self.config.base_font_size,
                bullet_color,
            );
            scene.draw_text(bullet_run);

            // Render item content
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
        let indent = theme::spacing::XL * 2.0; // More indent for number + space
        let number_x = origin.x;
        let margin = theme::spacing::XS;

        let mut y = origin.y + margin;

        for (i, item) in items.iter().enumerate() {
            let item_y = y;
            let number = start + i as u64;

            // Render number with opacity
            let number_color = self.config.text_color.with_alpha(self.config.text_color.a * opacity);
            let number_run = text_system.layout(
                &format!("{}.", number),
                Point::new(number_x, item_y),
                self.config.base_font_size,
                number_color,
            );
            scene.draw_text(number_run);

            // Render item content with more indent
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
        scene.draw_quad(
            Quad::new(Bounds::new(origin.x, origin.y + margin, max_width, 1.0))
                .with_background(rule_color),
        );

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

        // Header row
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
        y += 32.0; // Fixed header height

        // Header border with opacity
        let border_color = theme::border::DEFAULT.with_alpha(opacity);
        scene.draw_quad(
            Quad::new(Bounds::new(origin.x, y, max_width, 1.0))
                .with_background(border_color),
        );
        y += 1.0;

        // Data rows
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
            y += 28.0; // Fixed row height
        }

        y - origin.y
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to render markdown string directly.
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
