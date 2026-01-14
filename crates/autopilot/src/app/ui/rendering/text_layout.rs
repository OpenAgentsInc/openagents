fn byte_offset_for_char_index(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| text.len())
}

fn char_index_for_byte_offset(text: &str, byte_offset: usize) -> usize {
    let clamped = byte_offset.min(text.len());
    text[..clamped].chars().count()
}

fn prefix_text_for_line(prefix: &LinePrefix, text_system: &mut TextSystem) -> (String, f32) {
    let font_style = wgpui::text::FontStyle::default();
    let prefix_width = text_system.measure_styled_mono(&prefix.text, prefix.font_size, font_style);
    let space_width = text_system
        .measure_styled_mono(" ", prefix.font_size, font_style)
        .max(1.0);
    let gap_px = (prefix.content_x - prefix.x - prefix_width).max(space_width);
    let space_count = (gap_px / space_width).round().max(1.0) as usize;
    let mut text = prefix.text.clone();
    text.push_str(&" ".repeat(space_count));
    let total_width = prefix_width + space_width * space_count as f32;
    (text, total_width)
}

fn line_text_from_styled(lines: &[StyledLine]) -> String {
    let mut out = String::new();
    for (line_idx, line) in lines.iter().enumerate() {
        if line_idx > 0 {
            out.push('\n');
        }
        for span in &line.spans {
            out.push_str(&span.text);
        }
    }
    out
}

fn layout_styled_lines(
    lines: &[StyledLine],
    origin: Point,
    max_width: f32,
    base_indent: u32,
    text_system: &mut TextSystem,
    builder: &mut MessageLayoutBuilder,
    prefix: &mut Option<LinePrefix>,
) -> f32 {
    let mut y = origin.y;
    let mut first_visual_line = true;

    for line in lines {
        y += line.margin_top;
        let indent = (base_indent + line.indent) as f32 * wgpui::theme::spacing::LG;
        let line_start_x = origin.x + indent;
        let right_edge = origin.x + max_width;

        let base_font_size = line
            .spans
            .first()
            .map(|s| s.style.font_size)
            .unwrap_or(wgpui::theme::font_size::BASE);
        let line_height = base_font_size * line.line_height;

        let mut current_x = line_start_x;
        let mut line_x = line_start_x;
        let mut current_line_text = String::new();
        let mut line_has_text = false;
        let max_line_width = (right_edge - line_start_x).max(0.0);

        if first_visual_line {
            if let Some(prefix_line) = prefix.take() {
                let (prefix_text, prefix_width) = prefix_text_for_line(&prefix_line, text_system);
                line_x = prefix_line.x;
                current_x = prefix_line.x + prefix_width;
                current_line_text.push_str(&prefix_text);
            }
        }

        for span in &line.spans {
            let font_style = wgpui::text::FontStyle {
                bold: span.style.bold,
                italic: span.style.italic,
            };
            let words = split_into_words_for_layout(&span.text);
            let char_width = (span.style.font_size * 0.6).max(1.0);

            for word in words {
                if word.is_empty() {
                    continue;
                }

                let word_width =
                    text_system.measure_styled_mono(word, span.style.font_size, font_style);
                let available_width = right_edge - current_x;
                let needs_prefix_split =
                    !line_has_text && current_x > line_start_x && word_width > available_width;

                if word_width > max_line_width || needs_prefix_split {
                    if word_width > max_line_width && line_has_text {
                        builder.push_line(
                            current_line_text,
                            line_x,
                            y,
                            line_height,
                            base_font_size,
                        );
                        y += line_height;
                        current_line_text = String::new();
                        current_x = line_start_x;
                        line_x = line_start_x;
                        line_has_text = false;
                    }

                    let mut remaining = word;
                    let mut first_chunk = true;
                    while !remaining.is_empty() {
                        let width_for_chunk = if first_chunk && needs_prefix_split {
                            (right_edge - current_x).max(char_width)
                        } else {
                            max_line_width.max(char_width)
                        };
                        let max_chars =
                            (width_for_chunk / char_width).floor().max(1.0) as usize;
                        let remaining_chars = remaining.chars().count();
                        let end_ix = byte_offset_for_char_index(
                            remaining,
                            remaining_chars.min(max_chars),
                        );
                        let chunk = &remaining[..end_ix];
                        let chunk_width =
                            text_system.measure_styled_mono(chunk, span.style.font_size, font_style);

                        if current_x + chunk_width > right_edge && line_has_text {
                            builder.push_line(
                                current_line_text,
                                line_x,
                                y,
                                line_height,
                                base_font_size,
                            );
                            y += line_height;
                            current_line_text = String::new();
                            current_x = line_start_x;
                            line_x = line_start_x;
                            line_has_text = false;
                            first_chunk = false;
                            continue;
                        }

                        current_line_text.push_str(chunk);
                        current_x += chunk_width;
                        line_has_text = true;
                        remaining = &remaining[end_ix..];

                        if !remaining.is_empty() {
                            builder.push_line(
                                current_line_text,
                                line_x,
                                y,
                                line_height,
                                base_font_size,
                            );
                            y += line_height;
                            current_line_text = String::new();
                            current_x = line_start_x;
                            line_x = line_start_x;
                            line_has_text = false;
                        }
                        first_chunk = false;
                    }
                    continue;
                }

                if current_x + word_width > right_edge && current_x > line_start_x {
                    builder.push_line(
                        current_line_text,
                        line_x,
                        y,
                        line_height,
                        base_font_size,
                    );
                    y += line_height;
                    current_line_text = String::new();
                    current_x = line_start_x;
                    line_x = line_start_x;
                }

                current_line_text.push_str(word);
                current_x += word_width;
                line_has_text = true;
            }
        }

        builder.push_line(
            current_line_text,
            line_x,
            y,
            line_height,
            base_font_size,
        );
        y += line_height;
        first_visual_line = false;
    }

    y - origin.y
}

fn layout_markdown_block(
    block: &MarkdownBlock,
    origin: Point,
    max_width: f32,
    text_system: &mut TextSystem,
    config: &MarkdownConfig,
    builder: &mut MessageLayoutBuilder,
    prefix: &mut Option<LinePrefix>,
) -> f32 {
    match block {
        MarkdownBlock::Paragraph(lines) => {
            layout_styled_lines(lines, origin, max_width, 0, text_system, builder, prefix)
        }
        MarkdownBlock::Header { level, lines } => {
            let margin_top = match level {
                1 => wgpui::theme::spacing::XL,
                2 => wgpui::theme::spacing::LG,
                _ => wgpui::theme::spacing::MD,
            };
            margin_top
                + layout_styled_lines(
                    lines,
                    Point::new(origin.x, origin.y + margin_top),
                    max_width,
                    0,
                    text_system,
                    builder,
                    prefix,
                )
        }
        MarkdownBlock::CodeBlock { lines, .. } => {
            let margin = wgpui::theme::spacing::SM;
            let padding = wgpui::theme::spacing::SM;
            let header_height = wgpui::theme::font_size::XS + wgpui::theme::spacing::XS;

            let content_origin = Point::new(
                origin.x + padding,
                origin.y + margin + header_height + padding,
            );

            let content_height = layout_styled_lines(
                lines,
                content_origin,
                max_width - padding * 2.0,
                0,
                text_system,
                builder,
                prefix,
            );

            content_height + padding * 2.0 + header_height + margin * 2.0
        }
        MarkdownBlock::Blockquote(blocks) => {
            let bar_width = 4.0;
            let gap = wgpui::theme::spacing::MD;
            let indent = bar_width + gap;
            let margin = wgpui::theme::spacing::SM;
            let start_y = origin.y + margin;
            let mut y = start_y;

            for block in blocks {
                y += layout_markdown_block(
                    block,
                    Point::new(origin.x + indent, y),
                    max_width - indent,
                    text_system,
                    config,
                    builder,
                    prefix,
                );
            }

            y - start_y + margin * 2.0
        }
        MarkdownBlock::UnorderedList(items) => {
            let indent = wgpui::theme::spacing::XL;
            let bullet_x = origin.x + wgpui::theme::spacing::SM;
            let margin = wgpui::theme::spacing::XS;
            let mut y = origin.y + margin;

            for item in items {
                let mut item_prefix = Some(LinePrefix {
                    text: "\u{2022}".to_string(),
                    x: bullet_x,
                    content_x: origin.x + indent,
                    font_size: config.base_font_size,
                });
                for block in item {
                    y += layout_markdown_block(
                        block,
                        Point::new(origin.x + indent, y),
                        max_width - indent,
                        text_system,
                        config,
                        builder,
                        &mut item_prefix,
                    );
                }
            }

            y - origin.y + margin
        }
        MarkdownBlock::OrderedList { start, items } => {
            let indent = wgpui::theme::spacing::XL * 2.0;
            let margin = wgpui::theme::spacing::XS;
            let mut y = origin.y + margin;

            for (idx, item) in items.iter().enumerate() {
                let number = start + idx as u64;
                let mut item_prefix = Some(LinePrefix {
                    text: format!("{}.", number),
                    x: origin.x,
                    content_x: origin.x + indent,
                    font_size: config.base_font_size,
                });
                for block in item {
                    y += layout_markdown_block(
                        block,
                        Point::new(origin.x + indent, y),
                        max_width - indent,
                        text_system,
                        config,
                        builder,
                        &mut item_prefix,
                    );
                }
            }

            y - origin.y + margin
        }
        MarkdownBlock::HorizontalRule => {
            let margin = wgpui::theme::spacing::LG;
            margin * 2.0 + 1.0
        }
        MarkdownBlock::Table { headers, rows } => {
            if headers.is_empty() {
                return 0.0;
            }

            let cell_padding = wgpui::theme::spacing::SM;
            let mut y = origin.y + cell_padding;
            let header_text = headers
                .iter()
                .map(|cell| line_text_from_styled(cell))
                .collect::<Vec<_>>()
                .join(" | ");
            builder.push_line(
                header_text,
                origin.x + cell_padding,
                y,
                32.0,
                config.base_font_size,
            );
            y += 32.0 + 1.0;

            for row in rows {
                let row_text = row
                    .iter()
                    .map(|cell| line_text_from_styled(cell))
                    .collect::<Vec<_>>()
                    .join(" | ");
                builder.push_line(
                    row_text,
                    origin.x + cell_padding,
                    y + cell_padding,
                    28.0,
                    config.base_font_size,
                );
                y += 28.0;
            }

            y - origin.y
        }
    }
}

fn layout_markdown_document(
    document: &MarkdownDocument,
    origin: Point,
    max_width: f32,
    text_system: &mut TextSystem,
    config: &MarkdownConfig,
    builder: &mut MessageLayoutBuilder,
) -> f32 {
    let mut y = origin.y;

    for (i, block) in document.blocks.iter().enumerate() {
        if i > 0 {
            y += wgpui::theme::spacing::MD;
            builder.push_gap();
        }
        let mut prefix = None;
        y += layout_markdown_block(
            block,
            Point::new(origin.x, y),
            max_width,
            text_system,
            config,
            builder,
            &mut prefix,
        );
    }

    y - origin.y
}
