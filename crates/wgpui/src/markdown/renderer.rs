use crate::geometry::{Bounds, Point, Size};
use crate::scene::{Quad, Scene};
use crate::text::{FontStyle, TextSystem};
use crate::{Hsla, theme};

use super::types::*;

pub struct MarkdownRenderer {
    config: MarkdownConfig,
}

struct CodeBlockMetrics {
    margin: f32,
    padding: f32,
    header_height: f32,
    border_width: f32,
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self::with_config(MarkdownConfig::default())
    }

    pub fn with_config(config: MarkdownConfig) -> Self {
        Self { config }
    }

    fn code_block_metrics(&self) -> CodeBlockMetrics {
        CodeBlockMetrics {
            margin: theme::spacing::SM,
            padding: theme::spacing::SM,
            header_height: theme::font_size::XS + theme::spacing::XS,
            border_width: 1.0,
        }
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
            MarkdownBlock::Paragraph(lines) => {
                self.measure_lines(lines, max_width, 0, text_system)
            }

            MarkdownBlock::Header { level, lines } => {
                let margin_top = match level {
                    1 => theme::spacing::XL,
                    2 => theme::spacing::LG,
                    _ => theme::spacing::MD,
                };
                margin_top + self.measure_lines(lines, max_width, 0, text_system)
            }

            MarkdownBlock::CodeBlock { lines, .. } => {
                let metrics = self.code_block_metrics();
                let content_height: f32 = lines
                    .iter()
                    .map(|l| {
                        l.spans
                            .first()
                            .map(|s| s.style.font_size * l.line_height)
                            .unwrap_or(self.config.base_font_size * theme::line_height::NORMAL)
                    })
                    .sum();
                content_height
                    + metrics.padding * 2.0
                    + metrics.header_height
                    + metrics.margin * 2.0
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

    fn measure_lines(
        &self,
        lines: &[StyledLine],
        max_width: f32,
        base_indent: u32,
        text_system: &mut TextSystem,
    ) -> f32 {
        let mut height = 0.0;
        for line in lines {
            height += line.margin_top;
            let indent = (base_indent + line.indent) as f32 * theme::spacing::LG;
            let line_start_x = indent;
            let right_edge = max_width;
            let max_line_width = (right_edge - line_start_x).max(0.0);

            let base_font_size = line
                .spans
                .first()
                .map(|s| s.style.font_size)
                .unwrap_or(self.config.base_font_size);
            let line_height = base_font_size * line.line_height;

            let mut current_x = line_start_x;

            // Simulate word wrapping like render_lines does
            for span in &line.spans {
                let font_style = FontStyle {
                    bold: span.style.bold,
                    italic: span.style.italic,
                };

                let words = split_into_words(&span.text);

                for word in &words {
                    if word.is_empty() {
                        continue;
                    }

                    let word_width =
                        text_system.measure_styled_mono(word, span.style.font_size, font_style);

                    if word_width > max_line_width && max_line_width > 0.0 {
                        let char_width = (span.style.font_size * 0.6).max(1.0);
                        let max_chars =
                            (max_line_width / char_width).floor().max(1.0) as usize;
                        for chunk in split_long_word(word, max_chars) {
                            let chunk_width = text_system.measure_styled_mono(
                                &chunk,
                                span.style.font_size,
                                font_style,
                            );
                            if current_x + chunk_width > right_edge && current_x > line_start_x {
                                height += line_height;
                                current_x = line_start_x;
                            }
                            current_x += chunk_width;
                        }
                        continue;
                    }

                    // Check if word would wrap to next line
                    if current_x + word_width > right_edge && current_x > line_start_x {
                        height += line_height;
                        current_x = line_start_x;
                    }

                    current_x += word_width;
                }
            }

            // Add final line height
            height += line_height;
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

    pub fn render_with_layout(
        &self,
        document: &MarkdownDocument,
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
    ) -> MarkdownLayout {
        let mut layout = MarkdownLayout::default();
        layout.size = self.render_with_opacity_internal(
            document,
            origin,
            max_width,
            text_system,
            scene,
            1.0,
            Some(&mut layout.code_blocks),
        );
        layout
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
        self.render_with_opacity_internal(
            document,
            origin,
            max_width,
            text_system,
            scene,
            opacity,
            None,
        )
    }

    fn render_with_opacity_internal(
        &self,
        document: &MarkdownDocument,
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
        opacity: f32,
        mut code_blocks: Option<&mut Vec<CodeBlockLayout>>,
    ) -> Size {
        let mut y = origin.y;
        let x = origin.x;

        for (i, block) in document.blocks.iter().enumerate() {
            if i > 0 {
                y += theme::spacing::MD;
            }

            y += self.render_block_with_opacity(
                block,
                Point::new(x, y),
                max_width,
                text_system,
                scene,
                opacity,
                code_blocks.as_deref_mut(),
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
        code_blocks: Option<&mut Vec<CodeBlockLayout>>,
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

            MarkdownBlock::CodeBlock {
                lines, language, ..
            } => self.render_code_block(
                lines,
                language,
                origin,
                max_width,
                text_system,
                scene,
                opacity,
                code_blocks,
            ),

            MarkdownBlock::Blockquote(blocks) => self.render_blockquote(
                blocks,
                origin,
                max_width,
                text_system,
                scene,
                opacity,
                code_blocks,
            ),

            MarkdownBlock::UnorderedList(items) => self.render_unordered_list(
                items,
                origin,
                max_width,
                text_system,
                scene,
                opacity,
                code_blocks,
            ),

            MarkdownBlock::OrderedList { start, items } => self.render_ordered_list(
                *start,
                items,
                origin,
                max_width,
                text_system,
                scene,
                opacity,
                code_blocks,
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
            let line_start_x = origin.x + indent;
            let right_edge = origin.x + max_width;
            let max_line_width = (right_edge - line_start_x).max(0.0);

            let mut current_x = line_start_x;

            // Ensure minimum line height based on font size
            let base_font_size = line
                .spans
                .first()
                .map(|s| s.style.font_size)
                .unwrap_or(self.config.base_font_size);
            let line_height = base_font_size * line.line_height;

            for span in &line.spans {
                let font_style = FontStyle {
                    bold: span.style.bold,
                    italic: span.style.italic,
                };
                let color = span.style.color.with_alpha(span.style.color.a * opacity);

                // Split span text into words (keeping whitespace attached, newlines converted to spaces)
                let words = split_into_words(&span.text);

                for word in &words {
                    if word.is_empty() {
                        continue;
                    }

                    let word_width =
                        text_system.measure_styled_mono(word, span.style.font_size, font_style);

                    if word_width > max_line_width && max_line_width > 0.0 {
                        let char_width = (span.style.font_size * 0.6).max(1.0);
                        let max_chars =
                            (max_line_width / char_width).floor().max(1.0) as usize;
                        for chunk in split_long_word(word, max_chars) {
                            let chunk_width = text_system.measure_styled_mono(
                                &chunk,
                                span.style.font_size,
                                font_style,
                            );
                            if current_x + chunk_width > right_edge && current_x > line_start_x {
                                y += line_height;
                                current_x = line_start_x;
                            }

                            // Draw background if needed
                            if let Some(bg) = span.style.background {
                                let text_size =
                                    text_system.measure_size(&chunk, span.style.font_size, None);
                                let padding = 2.0;
                                let bg_with_opacity = bg.with_alpha(bg.a * opacity);
                                scene.draw_quad(Quad {
                                    bounds: Bounds::new(
                                        current_x - padding,
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

                            // Render chunk
                            let text_run = text_system.layout_styled_mono(
                                &chunk,
                                Point::new(current_x, y),
                                span.style.font_size,
                                color,
                                font_style,
                            );
                            scene.draw_text(text_run);

                            // Draw strikethrough if needed
                            if span.style.strikethrough {
                                let text_size =
                                    text_system.measure_size(&chunk, span.style.font_size, None);
                                let strike_y = y + text_size.height * 0.5;
                                scene.draw_quad(Quad {
                                    bounds: Bounds::new(current_x, strike_y, chunk_width, 1.0),
                                    background: Some(color),
                                    border_color: Hsla::transparent(),
                                    border_width: 0.0,
                                    corner_radius: 0.0,
                                });
                            }

                            current_x += chunk_width;
                        }
                        continue;
                    }

                    // Check if word fits on current line
                    if current_x + word_width > right_edge && current_x > line_start_x {
                        // Wrap to next line
                        y += line_height;
                        current_x = line_start_x;
                    }

                    // Draw background if needed
                    if let Some(bg) = span.style.background {
                        let text_size = text_system.measure_size(word, span.style.font_size, None);
                        let padding = 2.0;
                        let bg_with_opacity = bg.with_alpha(bg.a * opacity);
                        scene.draw_quad(Quad {
                            bounds: Bounds::new(
                                current_x - padding,
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

                    // Render word
                    let text_run = text_system.layout_styled_mono(
                        word,
                        Point::new(current_x, y),
                        span.style.font_size,
                        color,
                        font_style,
                    );
                    scene.draw_text(text_run);

                    // Draw strikethrough if needed
                    if span.style.strikethrough {
                        let text_size = text_system.measure_size(word, span.style.font_size, None);
                        let strike_y = y + text_size.height * 0.5;
                        scene.draw_quad(Quad {
                            bounds: Bounds::new(current_x, strike_y, word_width, 1.0),
                            background: Some(color),
                            border_color: Hsla::transparent(),
                            border_width: 0.0,
                            corner_radius: 0.0,
                        });
                    }

                    current_x += word_width;
                }
            }

            y += line_height;
        }

        y - origin.y
    }

    fn render_code_block(
        &self,
        lines: &[StyledLine],
        language: &Option<String>,
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
        opacity: f32,
        code_blocks: Option<&mut Vec<CodeBlockLayout>>,
    ) -> f32 {
        let metrics = self.code_block_metrics();

        let content_height: f32 = lines
            .iter()
            .map(|l| {
                l.spans
                    .first()
                    .map(|s| s.style.font_size * l.line_height)
                    .unwrap_or(self.config.base_font_size * theme::line_height::NORMAL)
            })
            .sum();

        let total_height = content_height + metrics.padding * 2.0 + metrics.header_height;

        let bg_color = self
            .config
            .code_background
            .with_alpha(self.config.code_background.a * opacity);
        let border_color = theme::border::DEFAULT.with_alpha(opacity);
        let block_bounds =
            Bounds::new(origin.x, origin.y + metrics.margin, max_width, total_height);
        scene.draw_quad(
            Quad::new(block_bounds)
                .with_background(bg_color)
                .with_border(border_color, metrics.border_width),
        );

        let header_bounds = Bounds::new(
            origin.x,
            origin.y + metrics.margin,
            max_width,
            metrics.header_height,
        );
        scene.draw_quad(
            Quad::new(header_bounds).with_background(theme::bg::SURFACE.with_alpha(opacity)),
        );
        scene.draw_quad(
            Quad::new(Bounds::new(
                header_bounds.origin.x,
                header_bounds.origin.y + header_bounds.size.height - metrics.border_width,
                header_bounds.size.width,
                metrics.border_width,
            ))
            .with_background(border_color),
        );

        if let Some(lang) = language.as_ref() {
            let label_x = header_bounds.origin.x + metrics.padding;
            let label_y = header_bounds.origin.y + header_bounds.size.height * 0.5
                - theme::font_size::XS * 0.55;
            let label_color = theme::text::MUTED.with_alpha(opacity);
            let label = text_system.layout(
                lang,
                Point::new(label_x, label_y),
                theme::font_size::XS,
                label_color,
            );
            scene.draw_text(label);
        }

        let content_origin = Point::new(
            origin.x + metrics.padding,
            origin.y + metrics.margin + metrics.header_height + metrics.padding,
        );

        self.render_lines(
            lines,
            content_origin,
            max_width - metrics.padding * 2.0,
            0,
            text_system,
            scene,
            opacity,
        );

        if let Some(code_blocks) = code_blocks {
            let content_bounds = Bounds::new(
                content_origin.x,
                content_origin.y,
                max_width - metrics.padding * 2.0,
                content_height,
            );
            code_blocks.push(CodeBlockLayout {
                bounds: block_bounds,
                header_bounds,
                content_bounds,
                language: language.clone(),
                code: collect_code_text(lines),
                copy_bounds: None,
            });
        }

        total_height + metrics.margin * 2.0
    }

    fn render_blockquote(
        &self,
        blocks: &[MarkdownBlock],
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
        opacity: f32,
        mut code_blocks: Option<&mut Vec<CodeBlockLayout>>,
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
                code_blocks.as_deref_mut(),
            );
        }

        let content_height = y - start_y;

        // Draw bar aligned with text visual top (accounting for text ascent)
        let bar_color = theme::text::PRIMARY.with_alpha(opacity);
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
        mut code_blocks: Option<&mut Vec<CodeBlockLayout>>,
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
            let bullet_run = text_system.layout_styled_mono(
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
                    code_blocks.as_deref_mut(),
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
        mut code_blocks: Option<&mut Vec<CodeBlockLayout>>,
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
            let number_run = text_system.layout_styled_mono(
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
                    code_blocks.as_deref_mut(),
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

fn collect_code_text(lines: &[StyledLine]) -> String {
    let mut out = String::new();
    for (line_ix, line) in lines.iter().enumerate() {
        if line_ix > 0 {
            out.push('\n');
        }
        for span in &line.spans {
            out.push_str(&span.text);
        }
    }
    out
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

/// Split text into words, keeping spaces attached to preceding word.
/// Newlines are converted to spaces to prevent cosmic_text from creating line breaks.
/// This is optimized for word wrapping - each segment can be wrapped independently.
fn split_into_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current_word = String::new();

    for c in text.chars() {
        if c == '\n' || c == '\r' {
            // Convert newlines to spaces, attach to current word if we have content
            if !current_word.is_empty() {
                current_word.push(' ');
            }
        } else if c.is_whitespace() {
            // Regular whitespace (spaces, tabs) - attach to current word
            current_word.push(c);
        } else {
            // Non-whitespace character
            if !current_word.is_empty()
                && current_word
                    .chars()
                    .last()
                    .map_or(false, |c| c.is_whitespace())
            {
                // Previous segment ended with whitespace, push it and start new word
                words.push(std::mem::take(&mut current_word));
            }
            current_word.push(c);
        }
    }

    // Push remaining text
    if !current_word.is_empty() {
        words.push(current_word);
    }

    words
}

fn byte_offset_for_char_index(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| text.len())
}

fn split_long_word(word: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 || word.is_empty() {
        return vec![word.to_string()];
    }
    let total_chars = word.chars().count();
    if total_chars <= max_chars {
        return vec![word.to_string()];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < total_chars {
        let end = (start + max_chars).min(total_chars);
        let start_ix = byte_offset_for_char_index(word, start);
        let end_ix = byte_offset_for_char_index(word, end);
        chunks.push(word[start_ix..end_ix].to_string());
        start = end;
    }
    if chunks.is_empty() {
        chunks.push(word.to_string());
    }
    chunks
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
