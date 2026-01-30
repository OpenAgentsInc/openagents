use std::ops::Range;

use wgpui::text_system::{LineFragment, LineWrapper};

use crate::{Position, TextBuffer};

#[derive(Clone, Debug)]
pub struct DisplayLine {
    pub buffer_line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

#[derive(Clone, Debug, Default)]
pub struct DisplayMap {
    lines: Vec<DisplayLine>,
    line_ranges: Vec<Range<usize>>,
    last_revision: u64,
    last_wrap_width: Option<f32>,
    last_char_width: f32,
}

impl DisplayMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn display_line_count(&self) -> usize {
        self.lines.len().max(1)
    }

    pub fn line(&self, index: usize) -> Option<&DisplayLine> {
        self.lines.get(index)
    }

    pub fn buffer_line_range(&self, line: usize) -> Option<Range<usize>> {
        self.line_ranges.get(line).cloned()
    }

    pub fn display_line_for_position(&self, position: Position) -> Option<usize> {
        let range = self.line_ranges.get(position.line)?;
        let mut last_index = None;
        for idx in range.clone() {
            let line = &self.lines[idx];
            last_index = Some(idx);
            if position.column < line.end_col {
                return Some(idx);
            }
        }
        last_index
    }

    pub fn update(
        &mut self,
        buffer: &TextBuffer,
        revision: u64,
        wrap_width: Option<f32>,
        char_width: f32,
    ) {
        let wrap_width = wrap_width.filter(|width| *width > 0.0 && char_width > 0.0);
        let same_revision = self.last_revision == revision;
        let same_wrap = self.last_wrap_width == wrap_width;
        let same_char_width = (self.last_char_width - char_width).abs() < 0.01;

        if same_revision && same_wrap && same_char_width {
            return;
        }

        self.lines.clear();
        self.line_ranges.clear();

        let line_count = buffer.line_count();
        for line_idx in 0..line_count {
            let line_text = buffer.line_text(line_idx);
            let line_len = line_text.chars().count();
            let start_index = self.lines.len();

            if let Some(width) = wrap_width {
                let mut wrapper = LineWrapper::new_monospace(0, 0.0, char_width);
                let fragments = [LineFragment::text(&line_text)];
                let mut last_col = 0usize;

                for boundary in wrapper.wrap_line(&fragments, width) {
                    let end_col = column_for_byte(&line_text, boundary.ix);
                    if end_col > last_col {
                        self.lines.push(DisplayLine {
                            buffer_line: line_idx,
                            start_col: last_col,
                            end_col,
                        });
                        last_col = end_col;
                    }
                }

                if last_col < line_len || line_len == 0 {
                    self.lines.push(DisplayLine {
                        buffer_line: line_idx,
                        start_col: last_col,
                        end_col: line_len,
                    });
                }
            } else {
                self.lines.push(DisplayLine {
                    buffer_line: line_idx,
                    start_col: 0,
                    end_col: line_len,
                });
            }

            let end_index = self.lines.len();
            self.line_ranges.push(start_index..end_index);
        }

        if self.lines.is_empty() {
            self.lines.push(DisplayLine {
                buffer_line: 0,
                start_col: 0,
                end_col: 0,
            });
            self.line_ranges.push(0..1);
        }

        self.last_revision = revision;
        self.last_wrap_width = wrap_width;
        self.last_char_width = char_width;
    }
}

fn column_for_byte(text: &str, byte: usize) -> usize {
    let slice_end = byte.min(text.len());
    text[..slice_end].chars().count()
}
