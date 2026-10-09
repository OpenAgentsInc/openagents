//! Lays out one transcript row into a display list. The measurements, sizes,
//! and spacing here are the transcript's design; adapters only paint them.

use super::Typography;
use super::display::{
    Accessibility, ColorRole, Font, Ink, Link, Rect, RowDisplay, Run, Scroller, TextStyle, Weight,
    Widget, WidgetKind,
};
use super::measure::{MeasureCache, MeasureRun, Measurer};
use crate::markdown::{self, Align, Block, Item, Span};
use crate::style::{Color, Space};
use crate::view::{Element, MessageRole, Node, TextRole, ToolState};

/// The earlier control's row key. Node keys are ASCII identifiers, so it
/// cannot collide with one.
pub const EARLIER_KEY: &str = "\u{1}earlier";

/// The widest a row's content grows; wider viewports center it.
pub const READING_WIDTH: f32 = 720.0;
/// The smallest side margin.
pub const SIDE_MARGIN: f32 = 16.0;
/// The widest a line inside a sideways scroller grows before it is
/// truncated.
pub const MAX_SCROLL_WIDTH: f32 = 8_192.0;
/// The widest a table column grows, at the default text size, before its
/// cells wrap.
pub const MAX_COLUMN_WIDTH: f32 = 260.0;
/// The widest an adapter-drawn surface in a transcript grows.
pub const SURFACE_WIDTH: f32 = 360.0;
/// The tallest an adapter-drawn surface in a transcript grows.
pub const SURFACE_HEIGHT: f32 = 480.0;

/// The horizontal band a row's content occupies at `width`.
pub fn content_band(width: f32) -> (f32, f32) {
    let side = SIDE_MARGIN.max((width - READING_WIDTH) / 2.0);
    (side, (width - 2.0 * side).max(1.0))
}

#[derive(Clone, Copy, PartialEq)]
enum Wrap {
    At(f32),
    /// One line per hard break; runs past `limit` are truncated.
    Clip(Option<f32>),
}

#[derive(Clone, Copy, PartialEq)]
enum AlignX {
    Start,
    Center,
    End,
}

/// One styled range of a paragraph. `code` draws an inline-code background;
/// `link` records the destination, which a `link` widget opens when
/// [`crate::markdown::opens`] admits it.
struct Piece {
    style: TextStyle,
    start: usize,
    end: usize,
    code: bool,
    link: Option<String>,
}

#[derive(Default)]
struct Para {
    text: String,
    pieces: Vec<Piece>,
}

impl Para {
    fn push(&mut self, text: &str, style: TextStyle, code: bool, link: Option<&str>) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        if let Some(last) = self.pieces.last_mut()
            && last.style.same(&style)
            && last.code == code
            && last.link.as_deref() == link
        {
            last.end = self.text.len();
            return;
        }
        self.pieces.push(Piece {
            style,
            start,
            end: self.text.len(),
            code,
            link: link.map(str::to_owned),
        });
    }

    fn plain(text: &str, style: TextStyle) -> Self {
        let mut para = Self::default();
        para.push(text, style, false, None);
        para
    }
}

pub(crate) struct Ctx<'a> {
    pub measurer: &'a mut dyn Measurer,
    pub cache: &'a mut MeasureCache,
    pub typography: &'a Typography,
    pub out: RowDisplay,
}

const PRIMARY: Ink = Ink::Role(ColorRole::Primary);
const SECONDARY: Ink = Ink::Role(ColorRole::Secondary);

fn ink(color: Option<Color>, tone: Ink) -> Ink {
    color.map_or(tone, |c| Ink::Rgba([c.red, c.green, c.blue, c.alpha]))
}

fn rect(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Rect {
    Rect {
        x,
        y,
        w,
        h,
        radii: [radius; 4],
        fill: None,
        stroke: None,
    }
}

impl Ctx<'_> {
    fn content_band(&self, width: f32) -> (f32, f32) {
        let side =
            SIDE_MARGIN.max((width - f32::from(self.typography.metrics.reading_width)) / 2.0);
        (side, (width - 2.0 * side).max(1.0))
    }
    fn body_size(&self) -> f32 {
        f32::from(self.typography.metrics.body_size)
    }
    fn font(&self, size: f32, weight: Weight, italic: bool, mono: bool) -> Font {
        Font {
            family: self.typography.family,
            size: self.typography.size(size),
            weight,
            italic,
            mono,
        }
    }

    fn style(&self, size: f32, weight: Weight, ink: Ink) -> TextStyle {
        TextStyle::new(self.font(size, weight, false, false), ink)
    }

    fn widget(&mut self, x: f32, y: f32, w: f32, h: f32, kind: WidgetKind) {
        self.out.widgets.push(Widget { x, y, w, h, kind });
    }

    /// Lays out `para` with its top at `y` and returns its height and widest
    /// line.
    fn para(
        &mut self,
        para: &Para,
        x: f32,
        y: f32,
        wrap: Wrap,
        spacing: f32,
        align: AlignX,
    ) -> (f32, f32) {
        self.para_with_height(para, x, y, wrap, spacing, align, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn para_with_height(
        &mut self,
        para: &Para,
        x: f32,
        y: f32,
        wrap: Wrap,
        spacing: f32,
        align: AlignX,
        line_height: Option<f32>,
    ) -> (f32, f32) {
        if para.text.is_empty() {
            return (0.0, 0.0);
        }
        // UTF-16 index to UTF-8 byte offset, with one entry past the end.
        let mut bytes16 = Vec::with_capacity(para.text.len() + 1);
        for (at, ch) in para.text.char_indices() {
            for _ in 0..ch.len_utf16() {
                bytes16.push(at as u32);
            }
        }
        bytes16.push(para.text.len() as u32);
        let to16 = |byte: usize| bytes16.partition_point(|b| (*b as usize) < byte) as u32;
        let runs: Vec<MeasureRun> = para
            .pieces
            .iter()
            .map(|p| MeasureRun {
                font: p.style.font,
                start16: to16(p.start),
                end16: to16(p.end),
            })
            .collect();
        let (width, limit) = match wrap {
            Wrap::At(w) => (Some(w.max(1.0)), None),
            Wrap::Clip(limit) => (None, limit),
        };
        let measured = self
            .cache
            .measure(&mut *self.measurer, &para.text, &runs, width);
        let text = self.out.texts.len() as u32;
        self.out.texts.push(para.text.clone());
        let styles: Vec<u32> = para
            .pieces
            .iter()
            .map(|p| self.out.style(p.style))
            .collect();
        let units: Vec<u16> = para.text.encode_utf16().collect();
        let band = width.or(limit).unwrap_or(0.0);
        let mut offsets = measured.offsets.iter();
        let mut top = y;
        let mut widest: f32 = 0.0;
        for (number, line) in measured.lines.iter().enumerate() {
            let fixed_body = self.typography.metrics.body_line_height > 0
                && runs.iter().any(|run| {
                    !run.font.mono
                        && (run.font.size - self.typography.size(self.body_size())).abs() < 0.01
                })
                && runs
                    .iter()
                    .all(|run| run.font.size <= self.typography.size(self.body_size()) + 0.01);
            let fixed_height = line_height.is_some() || fixed_body;
            if number > 0 && !fixed_height {
                top += spacing;
            }
            let inner: Vec<(u32, f32)> =
                super::measure::inner_boundaries(&runs, line.start16, line.end16)
                    .map(|b| (b, offsets.next().copied().unwrap_or(line.width)))
                    .collect();
            let x_at = |b: u32| {
                if b <= line.start16 {
                    0.0
                } else if b >= line.end16 {
                    line.width
                } else {
                    inner
                        .iter()
                        .find(|(at, _)| *at == b)
                        .map_or(line.width, |(_, x)| *x)
                }
            };
            let shown = match limit {
                Some(limit) => line.width.min(limit),
                None => line.width,
            };
            let dx = match align {
                AlignX::Start => 0.0,
                AlignX::Center => ((band - shown) / 2.0).max(0.0),
                AlignX::End => (band - shown).max(0.0),
            };
            let measured_height = line.ascent + line.descent + line.leading;
            let height = if fixed_height {
                self.typography.size(
                    line_height.unwrap_or(f32::from(self.typography.metrics.body_line_height)),
                )
            } else {
                measured_height
            };
            let baseline = top + line.ascent + ((height - measured_height) / 2.0).max(0.0);
            widest = widest.max(shown);
            for (index, run) in runs.iter().enumerate() {
                let start = run.start16.max(line.start16);
                let end = run.end16.min(line.end16);
                let mut visible = end;
                while visible > start && matches!(units[visible as usize - 1], 0x0A | 0x0D) {
                    visible -= 1;
                }
                if visible <= start {
                    continue;
                }
                let x0 = x_at(start);
                let x1 = x_at(end).max(x0);
                if let Some(limit) = limit
                    && x0 >= limit
                {
                    continue;
                }
                let truncate = limit.filter(|limit| x1 > *limit).map(|limit| limit - x0);
                let piece = &para.pieces[index];
                let run_x = x + dx + x0;
                let run_width = truncate.unwrap_or(x1 - x0);
                if piece.code {
                    let code = self.typography.metrics.markdown.and_then(|m| m.inline_code);
                    let inset = code
                        .map_or(0.0, |c| self.typography.size(f32::from(c.inset_y)))
                        .min(height / 2.0);
                    let radius = code.map_or(4.0, |c| {
                        self.typography.size(f32::from(c.radius_half_points) / 2.0)
                    });
                    let mut background = rect(
                        run_x - 2.0,
                        top + inset,
                        run_width + 4.0,
                        height - 2.0 * inset,
                        radius,
                    );
                    background.fill = Some(Ink::Role(ColorRole::InlineCode));
                    self.out.rects.push(background);
                }
                if let Some(destination) = &piece.link {
                    self.out.links.push(Link {
                        x: run_x,
                        y: top,
                        w: run_width,
                        h: height,
                        destination: destination.clone(),
                    });
                }
                self.out.runs.push(Run {
                    text,
                    start16: start,
                    len16: visible - start,
                    start8: bytes16[start as usize],
                    len8: bytes16[visible as usize] - bytes16[start as usize],
                    x: run_x,
                    baseline,
                    width: run_width,
                    style: styles[index],
                    truncate,
                });
            }
            top += height;
        }
        (top - y, widest)
    }

    /// Inline Markdown spans as one paragraph.
    fn spans(&self, spans: &[Span], size: f32, weight: Weight, ink: Ink, opacity: f32) -> Para {
        let mut para = Para::default();
        for span in spans {
            let code = self.typography.metrics.markdown.and_then(|m| m.inline_code);
            let strong = self
                .typography
                .metrics
                .markdown
                .and_then(|m| m.strong_weight);
            let emphasis = if span.bold {
                let strong = strong.unwrap_or(Weight::Bold);
                if weight as u8 > strong as u8 {
                    weight
                } else {
                    strong
                }
            } else {
                weight
            };
            let font = if span.code {
                self.font(
                    size * code.map_or(0.9, |c| f32::from(c.size_percent) / 100.0),
                    if span.bold && strong.is_none() {
                        Weight::Semibold
                    } else {
                        emphasis
                    },
                    span.italic,
                    true,
                )
            } else {
                self.font(size, emphasis, span.italic, false)
            };
            let mut style = TextStyle::new(
                font,
                if span.code
                    && let Some(code) = code
                {
                    Ink::Rgba(code.color)
                } else if span.link.is_some() {
                    Ink::Role(ColorRole::Link)
                } else {
                    ink
                },
            );
            style.opacity = opacity;
            style.underline = span.link.is_some();
            style.strike = span.strike;
            para.push(&span.text, style, span.code, span.link.as_deref());
        }
        para
    }

    fn blocks(&mut self, blocks: &[Block], x: f32, y: f32, w: f32, ink: Ink, spacing: f32) -> f32 {
        let mut top = y;
        for (index, block) in blocks.iter().enumerate() {
            if index > 0 {
                top += spacing;
            }
            top += self.block(block, x, top, w, ink);
        }
        top - y
    }

    fn block(&mut self, block: &Block, x: f32, y: f32, w: f32, ink: Ink) -> f32 {
        match block {
            Block::Heading { level, spans } => {
                if let Some(metrics) = self.typography.metrics.markdown {
                    let [size, height] =
                        metrics.headings[usize::from(*level).saturating_sub(1).min(3)];
                    let para = self.spans(spans, f32::from(size), Weight::Semibold, ink, 1.0);
                    return self
                        .para_with_height(
                            &para,
                            x,
                            y,
                            Wrap::At(w),
                            0.0,
                            AlignX::Start,
                            Some(f32::from(height)),
                        )
                        .0;
                }
                let (size, weight) = match level {
                    1 => (22.0, Weight::Bold),
                    2 => (19.0, Weight::Bold),
                    3 => (17.0, Weight::Semibold),
                    _ => (16.0, Weight::Semibold),
                };
                let pad = if *level <= 2 { 4.0 } else { 2.0 };
                let para = self.spans(spans, size, weight, ink, 1.0);
                pad + self
                    .para(&para, x, y + pad, Wrap::At(w), 2.0, AlignX::Start)
                    .0
            }
            Block::Paragraph { spans } => {
                let para = self.spans(spans, self.body_size(), Weight::Regular, ink, 1.0);
                self.para(&para, x, y, Wrap::At(w), 3.0, AlignX::Start).0
            }
            Block::List {
                ordered,
                start,
                items,
            } => self.list(*ordered, *start, items, x, y, w, ink),
            Block::Code { language, text } => self.code(language.as_deref(), text, x, y, w, ink),
            Block::Quote { blocks } => {
                let mark = self.out.mark();
                let height =
                    self.blocks(blocks, x + 12.0, y, (w - 12.0).max(1.0), ink.quieter(), 8.0);
                let mut bar = rect(x, y, 3.0, height, 1.5);
                bar.fill = Some(Ink::Role(ColorRole::Border));
                self.out.insert_rect(mark.rects, bar);
                height
            }
            Block::Table {
                align,
                header,
                rows,
            } => self.table(align, header, rows, x, y, w, ink),
            Block::Rule => {
                let mut rule = rect(x, y + 4.0, w, 1.0, 0.0);
                rule.fill = Some(Ink::Role(ColorRole::Border));
                self.out.rects.push(rule);
                9.0
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn list(
        &mut self,
        ordered: bool,
        start: u64,
        items: &[Item],
        x: f32,
        y: f32,
        w: f32,
        ink: Ink,
    ) -> f32 {
        let markers: Vec<Option<Para>> = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                if item.checked.is_some() {
                    return None;
                }
                let (text, weight) = if ordered {
                    (
                        format!("{}.", start.saturating_add(index as u64)),
                        Weight::Regular,
                    )
                } else {
                    ("•".to_owned(), Weight::Bold)
                };
                let mut style = self.style(self.body_size(), weight, ink);
                style.opacity = 0.7;
                Some(Para::plain(&text, style))
            })
            .collect();
        let mut column: f32 = 18.0;
        for marker in markers.iter().flatten() {
            let text = marker.text.clone();
            let runs = [MeasureRun {
                font: marker.pieces[0].style.font,
                start16: 0,
                end16: text.encode_utf16().count() as u32,
            }];
            let measured = self.cache.measure(&mut *self.measurer, &text, &runs, None);
            column = column.max(
                measured
                    .lines
                    .iter()
                    .map(|l| l.width)
                    .fold(0.0, f32::max)
                    .ceil(),
            );
        }
        let content_x = x + column + 8.0;
        let content_w = (w - column - 8.0).max(1.0);
        let mut top = y;
        for (index, (item, marker)) in items.iter().zip(&markers).enumerate() {
            if index > 0 {
                top += 6.0;
            }
            let marker_h = match (marker, item.checked) {
                (Some(marker), _) => {
                    self.para(marker, x, top, Wrap::Clip(Some(column)), 0.0, AlignX::End)
                        .0
                }
                (None, Some(checked)) => {
                    self.widget(
                        x + column - 16.0,
                        top,
                        16.0,
                        20.0,
                        WidgetKind::Checkbox { checked },
                    );
                    20.0
                }
                (None, None) => 0.0,
            };
            let content_h = self.blocks(&item.blocks, content_x, top, content_w, ink, 6.0);
            top += marker_h.max(content_h);
        }
        top - y
    }

    #[allow(clippy::too_many_arguments)]
    fn code(
        &mut self,
        language: Option<&str>,
        text: &str,
        x: f32,
        y: f32,
        w: f32,
        ink: Ink,
    ) -> f32 {
        let code = text.strip_suffix('\n').unwrap_or(text);
        let metrics = self.typography.metrics.markdown;
        let copy_icon = metrics.is_some_and(|metrics| metrics.copy_icon);
        let code = if code.is_empty() { " " } else { code };
        let mut frame = rect(x, y, w, 0.0, 10.0);
        frame.fill = Some(Ink::Role(ColorRole::Surface));
        frame.stroke = Some(Ink::Role(ColorRole::Border));
        let frame_index = self.out.rects.len();
        self.out.rects.push(frame);
        let label = language
            .filter(|l| !l.is_empty())
            .unwrap_or(if metrics.is_some() { "" } else { "code" });
        let label_size = metrics.map_or(12.0, |metrics| f32::from(metrics.code_label_size));
        let label_style = self.style(
            label_size,
            if metrics.is_some() {
                Weight::Regular
            } else {
                Weight::Semibold
            },
            SECONDARY,
        );
        let label_para = Para::plain(label, label_style);
        // The header and its Copy control grow with the caption size.
        let grow = self.typography.size(label_size) / label_size;
        let copy_w = ((if copy_icon { 24.0 } else { 72.0 }) * grow)
            .min(w / 2.0)
            .ceil();
        let mark = self.out.mark();
        let (label_h, _) = self.para(
            &label_para,
            x + if metrics.is_some() { 13.0 } else { 12.0 },
            y + if metrics.is_some() { 1.0 } else { 0.0 },
            Wrap::Clip(Some((w - 24.0 - copy_w).max(1.0))),
            0.0,
            AlignX::Start,
        );
        let header = metrics.map_or_else(
            || 32.0f32.max((label_h + 14.0).ceil()),
            |metrics| f32::from(metrics.code_header_height) * grow,
        );
        self.out
            .shift(mark, 0.0, ((header - label_h) / 2.0).max(0.0));
        self.widget(
            x + w - if copy_icon { 6.0 } else { 8.0 } - copy_w,
            y + if copy_icon {
                (header - 22.0 * grow) / 2.0
            } else {
                0.0
            },
            copy_w,
            if copy_icon { 22.0 * grow } else { header },
            WidgetKind::Copy {
                text: text.to_owned(),
                icon: copy_icon,
            },
        );
        let mut rule = rect(x, y + header, w, 1.0, 0.0);
        rule.fill = Some(Ink::Role(ColorRole::Border));
        self.out.rects.push(rule);
        let code_size = metrics.map_or(13.0, |metrics| {
            f32::from(metrics.code_size_half_points) / 2.0
        });
        let style = TextStyle::new(self.font(code_size, Weight::Regular, false, true), ink);
        let para = Para::plain(code, style);
        if let Some(language) = language {
            self.out.code_blocks.push(super::display::CodeBlock {
                text: self.out.texts.len() as u32,
                language: language.to_owned(),
            });
        }
        // Code keeps its lines; a block wider than the row scrolls sideways.
        let mark = self.out.mark();
        let padding_y = metrics.map_or(12.0, |metrics| f32::from(metrics.code_padding_y));
        let (text_h, widest) = self.para_with_height(
            &para,
            x + if metrics.is_some() { 13.0 } else { 12.0 },
            y + header + 1.0 + padding_y,
            Wrap::Clip(Some(MAX_SCROLL_WIDTH)),
            3.0,
            AlignX::Start,
            metrics.map(|metrics| f32::from(metrics.code_line_height)),
        );
        let height = header + if metrics.is_some() { 2.0 } else { 1.0 } + 2.0 * padding_y + text_h;
        self.out.rects[frame_index].h = height;
        // Inside the one-point border, below the header rule.
        self.scroller(
            mark,
            x + 1.0,
            y + header + 1.0,
            w - 2.0,
            height - header - 2.0,
            widest.ceil() + 23.0,
        );
        height
    }

    #[allow(clippy::too_many_arguments)]
    fn table(
        &mut self,
        align: &[Align],
        header: &[Vec<Span>],
        rows: &[Vec<Vec<Span>>],
        x: f32,
        y: f32,
        w: f32,
        ink: Ink,
    ) -> f32 {
        let columns = header.len().max(1);
        let cell = |ctx: &Self, spans: &[Span], head: bool| {
            ctx.spans(
                spans,
                15.0,
                if head {
                    Weight::Semibold
                } else {
                    Weight::Regular
                },
                ink,
                1.0,
            )
        };
        let empty: Vec<Span> = vec![];
        let cells_of = |row: &[Vec<Span>], column: usize| -> Vec<Span> {
            row.get(column).unwrap_or(&empty).clone()
        };
        let all: Vec<(bool, &[Vec<Span>])> = std::iter::once((true, header))
            .chain(rows.iter().map(|r| (false, r.as_slice())))
            .collect();
        // Natural column widths, then shrink wide columns to fit.
        let mut natural = vec![0.0f32; columns];
        for (head, row) in &all {
            for (column, width) in natural.iter_mut().enumerate() {
                let para = cell(self, &cells_of(row, column), *head);
                *width = width.max(self.natural(&para) + 20.0);
            }
        }
        // Columns wrap at a readable width. A table then no more than a
        // tenth wider than the row wraps its widest columns a little more;
        // one wider still scrolls sideways.
        let most = MAX_COLUMN_WIDTH * self.typography.size(15.0) / 15.0;
        let capped: Vec<f32> = natural.iter().map(|n| n.min(most)).collect();
        let capped_w = capped.iter().sum::<f32>();
        let widths = if capped_w <= w {
            fit_columns(&natural, w)
        } else if capped_w <= w * 1.1 {
            fit_columns(&capped, w)
        } else {
            capped
        };
        let table_w: f32 = widths.iter().sum();
        let mark = self.out.mark();
        let mut frame = rect(x, y, table_w, 0.0, 8.0);
        frame.fill = Some(Ink::Role(ColorRole::Surface));
        let frame_index = self.out.rects.len();
        self.out.rects.push(frame);
        let mut top = y;
        for (index, (head, row)) in all.iter().enumerate() {
            if index > 0 {
                let mut rule = rect(x, top, table_w, 1.0, 0.0);
                rule.fill = Some(Ink::Role(ColorRole::Border));
                self.out.rects.push(rule);
                top += 1.0;
            }
            let background = self.out.rects.len();
            let mut left = x;
            let mut height: f32 = 0.0;
            for (column, width) in widths.iter().enumerate() {
                let para = cell(self, &cells_of(row, column), *head);
                let alignment = match align.get(column) {
                    Some(Align::Center) => AlignX::Center,
                    Some(Align::Right) => AlignX::End,
                    _ => AlignX::Start,
                };
                let (h, _) = self.para(
                    &para,
                    left + 10.0,
                    top + 7.0,
                    Wrap::At((width - 20.0).max(1.0)),
                    2.0,
                    alignment,
                );
                height = height.max(h);
                left += width;
            }
            let height = height.max(self.line_height(15.0)) + 14.0;
            if *head {
                let mut raised = rect(x, top, table_w, height, 0.0);
                raised.radii = [8.0, 8.0, 0.0, 0.0];
                raised.fill = Some(Ink::Role(ColorRole::Raised));
                self.out.insert_rect(background, raised);
            }
            top += height;
        }
        let height = top - y;
        self.out.rects[frame_index].h = height;
        let mut border = rect(x, y, table_w, height, 8.0);
        border.stroke = Some(Ink::Role(ColorRole::Border));
        self.out.rects.push(border);
        self.scroller(mark, x, y, w, height, table_w);
        height
    }

    /// Makes the items added since `mark` scroll sideways inside
    /// `x..x + w`, when they are wider than that.
    fn scroller(
        &mut self,
        mark: super::display::Mark,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        content_w: f32,
    ) {
        if content_w <= w + 0.5 || w < 1.0 || h < 1.0 {
            return;
        }
        let out = &mut self.out;
        out.scrollers.push(Scroller {
            x,
            y,
            w,
            h,
            content_w: content_w.min(MAX_SCROLL_WIDTH + 64.0),
            runs: [mark.runs as u32, out.runs.len() as u32],
            rects: [mark.rects as u32, out.rects.len() as u32],
            links: [mark.links as u32, out.links.len() as u32],
        });
    }

    /// The widest line of `para` without wrapping.
    fn natural(&mut self, para: &Para) -> f32 {
        let mark = self.out.mark();
        let (_, widest) = self.para(para, 0.0, 0.0, Wrap::Clip(None), 0.0, AlignX::Start);
        self.out.truncate_to(mark);
        widest.ceil()
    }

    /// An estimated single-line height for an empty cell.
    fn line_height(&self, size: f32) -> f32 {
        self.typography.size(size) * 1.2
    }

    fn node(&mut self, node: &Node<()>, x: f32, y: f32, w: f32, tone: Ink, in_tool: bool) -> f32 {
        let ink = ink(node.style.foreground, tone);
        match &node.element {
            Element::Text { value, role } => {
                let (size, weight, mono, wrap) = match role {
                    TextRole::Terminal => (12.0, Weight::Regular, true, Wrap::Clip(Some(w))),
                    TextRole::Code if in_tool => (12.0, Weight::Regular, true, Wrap::At(w)),
                    TextRole::Code => (13.0, Weight::Regular, true, Wrap::At(w)),
                    TextRole::Heading => (17.0, Weight::Semibold, false, Wrap::At(w)),
                    TextRole::Status => (13.0, Weight::Regular, false, Wrap::At(w)),
                    TextRole::Body | TextRole::Markdown if in_tool => {
                        (13.0, Weight::Regular, false, Wrap::At(w))
                    }
                    TextRole::Body | TextRole::Markdown => {
                        (self.body_size(), Weight::Regular, false, Wrap::At(w))
                    }
                };
                let weight = match node.style.weight {
                    Some(crate::style::TextWeight::Bold) => Weight::Bold,
                    Some(crate::style::TextWeight::Medium) => Weight::Medium,
                    Some(crate::style::TextWeight::Semibold) => Weight::Semibold,
                    Some(crate::style::TextWeight::Normal) => Weight::Regular,
                    None => weight,
                };
                let style = TextStyle::new(self.font(size, weight, false, mono), ink);
                let value = value.strip_suffix('\n').unwrap_or(value);
                self.para(
                    &Para::plain(value, style),
                    x,
                    y,
                    wrap,
                    if mono { 2.0 } else { 3.0 },
                    match node.style.align {
                        Some(crate::style::TextAlign::Center) => AlignX::Center,
                        Some(crate::style::TextAlign::End) => AlignX::End,
                        _ => AlignX::Start,
                    },
                )
                .0
            }
            Element::Markdown { blocks } => self.blocks(blocks, x, y, w, ink, 12.0),
            Element::Message {
                role,
                note,
                children,
            } => self.message(*role, note.as_deref(), children, x, y, w, ink),
            Element::Tool {
                name,
                detail,
                state,
                children,
            } => self.tool(&node.key, name, detail, *state, children, false, x, y, w),
            Element::Working { label } => self.working(label, x, y),
            Element::Stack { children, .. } => {
                let spacing = |space: Option<Space>, default: f32| match space {
                    Some(Space::None) => 0.0,
                    Some(Space::Xs) => 4.0,
                    Some(Space::Sm) => 8.0,
                    Some(Space::Md) => 16.0,
                    Some(Space::Lg) => 24.0,
                    None => default,
                };
                let left = spacing(node.style.padding_start, 0.0).min(w / 4.0);
                let right = spacing(node.style.padding_end, 0.0).min(w / 4.0);
                let top = spacing(node.style.padding_top, 0.0);
                let bottom = spacing(node.style.padding_bottom, 0.0);
                let height =
                    top + self.stack(
                        children,
                        x + left,
                        y + top,
                        (w - left - right).max(1.0),
                        ink,
                        spacing(node.style.gap, 8.0),
                        in_tool,
                    ) + bottom;
                if let Some(background) = node.style.background {
                    let mut bounds = rect(x, y, w, height, 10.0);
                    bounds.fill = Some(Ink::Rgba([
                        background.red,
                        background.green,
                        background.blue,
                        background.alpha,
                    ]));
                    bounds.stroke = Some(Ink::Role(ColorRole::Border));
                    self.out.rects.insert(0, bounds);
                }
                height
            }
            Element::List { children, .. } | Element::Transcript { children, .. } => {
                self.stack(children, x, y, w, ink, 8.0, in_tool)
            }
            Element::Button {
                label,
                enabled,
                icon,
                ..
            } => {
                // A pill chip, or a button that keeps its measured width,
                // is as wide as its label, as the phone draws it; any
                // other button fills the row.
                let pill = icon.is_some_and(|icon| icon.pill && !icon.circular);
                let hug = pill || node.style.intrinsic_width == Some(true);
                let pad = if hug { 14.0_f32 } else { 10.0 }.min(w / 4.0);
                let style =
                    self.style(15.0, Weight::Medium, if *enabled { ink } else { SECONDARY });
                let para = Para::plain(label, style);
                let w = if hug {
                    (self.natural(&para) + 2.0 * pad).min(w)
                } else {
                    w
                };
                let height =
                    self.para(
                        &para,
                        x + pad,
                        y + 7.0,
                        Wrap::At((w - 2.0 * pad).max(1.0)),
                        2.0,
                        AlignX::Center,
                    )
                    .0 + 14.0;
                let radius = if pill { (height / 2.0).min(18.0) } else { 7.0 };
                let mut bounds = rect(x, y, w, height, radius);
                bounds.fill = Some(
                    node.style
                        .background
                        .map_or(Ink::Role(ColorRole::Raised), |c| {
                            Ink::Rgba([c.red, c.green, c.blue, c.alpha])
                        }),
                );
                bounds.stroke = Some(Ink::Role(ColorRole::Border));
                self.out.rects.push(bounds);
                self.widget(
                    x,
                    y,
                    w,
                    height,
                    WidgetKind::Button {
                        key: node.key.clone(),
                        enabled: *enabled,
                    },
                );
                height
            }
            // A surface with a height is drawn by the adapter in the box
            // reserved here, at most a reading card's width; one without
            // shows its label.
            Element::Surface { resource, label } if node.style.min_height.is_some() => {
                let h = f32::from(node.style.min_height.unwrap_or_default()).min(SURFACE_HEIGHT);
                let w = w.min(SURFACE_WIDTH);
                let mut bounds = rect(x, y, w, h, 14.0);
                bounds.fill = Some(Ink::Role(ColorRole::Raised));
                bounds.stroke = Some(Ink::Role(ColorRole::Border));
                self.out.rects.push(bounds);
                self.widget(
                    x,
                    y,
                    w,
                    h,
                    WidgetKind::Surface {
                        resource: resource.clone(),
                        label: label.clone(),
                    },
                );
                h
            }
            Element::Surface { label, .. } => {
                let style = self.style(15.0, Weight::Regular, SECONDARY);
                self.para(
                    &Para::plain(label, style),
                    x,
                    y,
                    Wrap::At(w),
                    2.0,
                    AlignX::Start,
                )
                .0
            }
            // v3 editing and focus scopes are mounted by their adapter,
            // outside the read-only transcript display list.
            Element::Composer { .. }
            | Element::Field { .. }
            | Element::Choice { .. }
            | Element::Dialog { .. } => 0.0,
            Element::RichText { runs, .. } => {
                let text: String = runs.iter().map(|run| run.text.as_str()).collect();
                let style = self.style(13.0, Weight::Regular, ink);
                self.para(
                    &Para::plain(&text, style),
                    x,
                    y,
                    Wrap::At(w),
                    2.0,
                    AlignX::Start,
                )
                .0
            }
        }
    }

    /// Children stacked vertically with `spacing` between those that drew.
    #[allow(clippy::too_many_arguments)]
    fn stack(
        &mut self,
        nodes: &[Node<()>],
        x: f32,
        y: f32,
        w: f32,
        tone: Ink,
        spacing: f32,
        in_tool: bool,
    ) -> f32 {
        let mut top = y;
        let mut drew = false;
        for node in nodes {
            let gap = if drew { spacing } else { 0.0 };
            let height = self.node(node, x, top + gap, w, tone, in_tool);
            if height > 0.0 {
                top += gap + height;
                drew = true;
            }
        }
        top - y
    }

    #[allow(clippy::too_many_arguments)]
    fn message(
        &mut self,
        role: MessageRole,
        note: Option<&str>,
        children: &[Node<()>],
        x: f32,
        y: f32,
        w: f32,
        tone: Ink,
    ) -> f32 {
        match role {
            MessageRole::User => {
                let metrics = self.typography.metrics;
                let padding = f32::from(metrics.bubble_padding);
                let cap = if metrics.bubble_max_percent == 0 {
                    (w - 48.0).max(1.0)
                } else {
                    w * f32::from(metrics.bubble_max_percent) / 100.0
                };
                let inner_w = (cap - 2.0 * padding).max(1.0);
                let inner_x = x + w - cap + padding;
                let mark = self.out.mark();
                let content_h = self.stack(children, inner_x, y + 10.0, inner_w, tone, 8.0, false);
                let wrapped = metrics.bubble_max_percent > 0
                    && self.out.runs[mark.runs..]
                        .first()
                        .zip(self.out.runs[mark.runs..].last())
                        .is_some_and(|(first, last)| first.baseline != last.baseline);
                let natural = if wrapped || self.out.has_blocks_since(mark) {
                    inner_w
                } else {
                    (self.out.text_extent(mark) - inner_x)
                        .ceil()
                        .clamp(0.0, inner_w)
                };
                self.out.shift(mark, inner_w - natural, 0.0);
                let bubble_w = natural + 2.0 * padding;
                let radius = f32::from(metrics.bubble_radius);
                let mut bubble = rect(x + w - bubble_w, y, bubble_w, content_h + 20.0, radius);
                bubble.radii = [
                    radius,
                    radius,
                    f32::from(metrics.bubble_tail_radius),
                    radius,
                ];
                bubble.fill = Some(Ink::Role(ColorRole::Bubble));
                self.out.insert_rect(mark.rects, bubble);
                let mut height = content_h + 20.0;
                if let Some(note) = note {
                    let style = self.style(12.0, Weight::Regular, SECONDARY);
                    height += 4.0;
                    height += self
                        .para(
                            &Para::plain(note, style),
                            x,
                            y + height,
                            Wrap::At(w),
                            2.0,
                            AlignX::End,
                        )
                        .0;
                }
                height
            }
            MessageRole::System => {
                let text = children
                    .iter()
                    .map(plain)
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                let text = match note {
                    Some(note) if !text.is_empty() => format!("{text} · {note}"),
                    Some(note) => note.to_owned(),
                    None => text,
                };
                let color = children.first().and_then(|c| match &c.element {
                    Element::Text { .. } => c.style.foreground,
                    _ => None,
                });
                let style = self.style(13.0, Weight::Regular, ink(color, SECONDARY));
                self.para(
                    &Para::plain(&text, style),
                    x,
                    y,
                    Wrap::At(w),
                    2.0,
                    AlignX::Center,
                )
                .0
            }
            MessageRole::Assistant => {
                let mut height = self.stack(children, x, y, w, tone, 10.0, false);
                if let Some(note) = note {
                    let style = self.style(12.0, Weight::Regular, SECONDARY);
                    height += 10.0;
                    height += self
                        .para(
                            &Para::plain(note, style),
                            x,
                            y + height,
                            Wrap::At(w),
                            2.0,
                            AlignX::Start,
                        )
                        .0;
                }
                height
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn tool(
        &mut self,
        key: &str,
        name: &str,
        detail: &str,
        state: ToolState,
        children: &[Node<()>],
        expanded: bool,
        x: f32,
        y: f32,
        w: f32,
    ) -> f32 {
        let header = 36.0f32;
        let chevron = if children.is_empty() { 0.0 } else { 20.0 };
        let name_style = self.style(15.0, Weight::Semibold, PRIMARY);
        let name_para = Para::plain(name, name_style);
        let name_x = x + 26.0;
        let room = (w - 26.0 - chevron).max(1.0);
        let mark = self.out.mark();
        let (name_h, name_w) = self.para(
            &name_para,
            name_x,
            y,
            Wrap::Clip(Some(room)),
            0.0,
            AlignX::Start,
        );
        let mut text_h = name_h;
        let detail_room = room - name_w - 8.0;
        if !detail.is_empty() && detail_room > 24.0 {
            let style = self.style(15.0, Weight::Regular, SECONDARY);
            let first_line = detail.lines().next().unwrap_or_default();
            let (detail_h, _) = self.para(
                &Para::plain(first_line, style),
                name_x + name_w + 8.0,
                y,
                Wrap::Clip(Some(detail_room)),
                0.0,
                AlignX::Start,
            );
            text_h = text_h.max(detail_h);
        }
        let header = header.max(text_h + 8.0);
        self.out.shift(mark, 0.0, (header - text_h) / 2.0);
        self.widget(
            x,
            y + (header - 18.0) / 2.0,
            18.0,
            18.0,
            WidgetKind::Status { state },
        );
        if !children.is_empty() {
            self.widget(
                x + w - 12.0,
                y + (header - 12.0) / 2.0,
                12.0,
                12.0,
                WidgetKind::Chevron { expanded },
            );
            self.widget(
                x,
                y,
                w,
                header,
                WidgetKind::Toggle {
                    key: key.to_owned(),
                    expanded,
                },
            );
        }
        if !expanded || children.is_empty() {
            return header;
        }
        let body_y = y + header + 4.0;
        let mut body = rect(x, body_y, w, 0.0, 8.0);
        body.fill = Some(Ink::Role(ColorRole::Surface));
        let body_index = self.out.rects.len();
        self.out.rects.push(body);
        let content = self.stack(
            children,
            x + 10.0,
            body_y + 10.0,
            (w - 20.0).max(1.0),
            SECONDARY,
            6.0,
            true,
        );
        self.out.rects[body_index].h = content + 20.0;
        header + 4.0 + content + 20.0
    }

    /// A working row: a spinning activity indicator, then its label.
    fn working(&mut self, label: &str, x: f32, y: f32) -> f32 {
        let style = self.style(15.0, Weight::Regular, SECONDARY);
        let (h, _) = self.para(
            &Para::plain(label, style),
            x + 26.0,
            y + 4.0,
            Wrap::Clip(None),
            0.0,
            AlignX::Start,
        );
        let h = h.max(self.line_height(15.0));
        self.widget(
            x,
            y + 4.0 + (h - 18.0) / 2.0,
            18.0,
            18.0,
            WidgetKind::Spinner,
        );
        h + 8.0
    }
}

/// Column widths that fit `available`: columns narrower than an even share
/// keep their natural width; the rest split what remains.
fn fit_columns(natural: &[f32], available: f32) -> Vec<f32> {
    let total: f32 = natural.iter().sum();
    if total <= available {
        return natural.to_vec();
    }
    let mut widths = vec![0.0; natural.len()];
    let mut open: Vec<usize> = (0..natural.len()).collect();
    let mut room = available;
    loop {
        let share = room / open.len().max(1) as f32;
        let (fixed, rest): (Vec<usize>, Vec<usize>) =
            open.iter().partition(|&&i| natural[i] <= share);
        if fixed.is_empty() {
            for i in &rest {
                widths[*i] = share.max(48.0);
            }
            return widths;
        }
        for i in fixed {
            widths[i] = natural[i];
            room -= natural[i];
        }
        if rest.is_empty() {
            return widths;
        }
        open = rest;
    }
}

/// The plain text of a node, for Copy and accessibility.
pub(crate) fn plain(node: &Node<()>) -> String {
    match &node.element {
        Element::Text { value, .. } => value.clone(),
        Element::RichText { runs, .. } => runs.iter().map(|run| run.text.as_str()).collect(),
        Element::Field { label, .. } | Element::Choice { label, .. } => label.clone(),
        Element::Markdown { blocks } => markdown::plain(blocks),
        Element::Button { label, .. }
        | Element::Working { label }
        | Element::Surface { label, .. } => label.clone(),
        Element::Tool {
            name,
            detail,
            children,
            ..
        } => std::iter::once(if detail.is_empty() {
            name.clone()
        } else {
            format!("{name} {detail}")
        })
        .chain(children.iter().map(plain))
        .collect::<Vec<_>>()
        .join("\n"),
        Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Message { children, .. }
        | Element::Dialog { children, .. }
        | Element::Transcript { children, .. } => children
            .iter()
            .map(plain)
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        Element::Composer { .. } => String::new(),
    }
}

/// Lays out one row at the top of a `width`-point band.
pub(crate) fn lay_row(ctx: &mut Ctx<'_>, node: &Node<()>, expanded: bool, width: f32) -> f32 {
    let (x, w) = ctx.content_band(width);
    let tone = ink(node.style.foreground, PRIMARY);
    let height = match &node.element {
        Element::Message {
            role,
            note,
            children,
        } => {
            let text = children
                .iter()
                .map(plain)
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            ctx.out.accessibility = Accessibility {
                label: match note {
                    Some(note) if !note.is_empty() => format!("{text}, {note}"),
                    _ => text.clone(),
                },
                ..Accessibility::default()
            };
            ctx.out.copy = Some(text);
            ctx.message(*role, note.as_deref(), children, x, 0.0, w, tone)
        }
        Element::Tool {
            name,
            detail,
            state,
            children,
        } => {
            let state_label = match state {
                ToolState::Running => "running",
                ToolState::Done => "done",
                ToolState::Failed => "failed",
            };
            ctx.out.accessibility = Accessibility {
                label: format!("{name}, {state_label}"),
                value: detail.clone(),
                hint: match (children.is_empty(), expanded) {
                    (true, _) => String::new(),
                    (false, true) => "Collapses the output".into(),
                    (false, false) => "Expands the output".into(),
                },
                button: !children.is_empty(),
            };
            ctx.tool(
                &node.key, name, detail, *state, children, expanded, x, 0.0, w,
            )
        }
        _ => {
            ctx.out.accessibility.label = plain(node);
            ctx.node(node, x, 0.0, w, tone, false)
        }
    };
    height.max(1.0).ceil()
}

/// Lays out the control that loads older rows.
pub(crate) fn lay_earlier(ctx: &mut Ctx<'_>, label: &str, loading: bool, width: f32) -> f32 {
    let (x, w) = ctx.content_band(width);
    let height = 44.0f32.max(ctx.line_height(15.0) + 16.0).ceil();
    let style = ctx.style(15.0, Weight::Regular, SECONDARY);
    let para = Para::plain(label, style);
    let label_w = ctx.natural(&para).min(w);
    let spinner = if loading { 24.0 } else { 0.0 };
    let left = x + ((w - label_w - spinner) / 2.0).max(0.0);
    if loading {
        ctx.widget(left, (height - 16.0) / 2.0, 16.0, 16.0, WidgetKind::Spinner);
    }
    let mark = ctx.out.mark();
    let (label_h, _) = ctx.para(
        &para,
        left + spinner,
        0.0,
        Wrap::Clip(Some(w - spinner)),
        0.0,
        AlignX::Start,
    );
    ctx.out.shift(mark, 0.0, (height - label_h) / 2.0);
    ctx.widget(x, 0.0, w, height, WidgetKind::Earlier { loading });
    ctx.out.accessibility = Accessibility {
        label: label.to_owned(),
        hint: if loading {
            String::new()
        } else {
            "Loads older messages".into()
        },
        button: !loading,
        ..Accessibility::default()
    };
    height
}
