//! Markdown drawn with GPUI elements from the block tree every OpenAgents
//! frontend shares (`openagents_markdown::parse_full`).
//!
//! The streaming veil follows the desktop's rule: new text fades in by
//! multiplying an alpha into its color at paint time, so shaping and wrapping
//! never change while a reply streams.

use std::ops::Range;

use gpui::{
    AnyElement, App, ElementId, FontWeight, Hsla, InteractiveText, IntoElement, ParentElement,
    SharedString, StrikethroughStyle, StyledText, TextRun, UnderlineStyle, Window, div, prelude::*,
    px,
};
use openagents_markdown::parser::TableAlign;
use openagents_markdown::{Block, InlineRun, parse_full, parse_streaming};

use crate::theme;

/// Fading chunks at the end of a streaming text part, oldest first: how many
/// visible characters each one added and its current alpha.
#[derive(Clone, Debug, Default)]
pub struct Veil {
    pub tail: Vec<(usize, f32)>,
}

impl Veil {
    fn is_empty(&self) -> bool {
        self.tail.is_empty()
    }
}

/// Characters a chunk of markdown source adds to the drawn text, roughly:
/// markup punctuation never draws.
pub fn visible_chars(chunk: &str) -> usize {
    chunk
        .chars()
        .filter(|c| !matches!(c, '*' | '`' | '#' | '|' | '>' | '[' | ']' | '_' | '~'))
        .count()
}

pub struct Markdown {
    /// Prefix for element ids, unique per text part.
    pub id: SharedString,
    pub color: Hsla,
}

impl Markdown {
    /// Draws `source`. While it streams, its unfinished tail is held back or
    /// closed for display (a table header waiting for its delimiter row, a
    /// fence's opening line, a bare `**`), so raw syntax never flashes
    /// (#12246).
    pub fn render(&self, source: &str, streaming: bool, veil: Option<&Veil>) -> AnyElement {
        let tree = if streaming {
            parse_streaming(source)
        } else {
            parse_full(source)
        };
        let count = tree.blocks.len();
        let mut counter = 0usize;
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .w_full()
            .min_w_0()
            .children(tree.blocks.iter().enumerate().map(|(ix, top)| {
                let veil = veil.filter(|_| ix + 1 == count);
                self.block(&top.block, veil, &mut counter)
            }))
            .into_any_element()
    }

    fn next_id(&self, counter: &mut usize) -> ElementId {
        *counter += 1;
        ElementId::Name(format!("{}-{}", self.id, counter).into())
    }

    fn block(&self, block: &Block, veil: Option<&Veil>, counter: &mut usize) -> AnyElement {
        match block {
            Block::Paragraph { runs } => self.inline(runs, None, veil, counter),
            Block::Heading { level, runs } => {
                let (size, line) = match level {
                    1 => (21., 28.),
                    2 => (18., 25.),
                    3 => (16., 23.),
                    _ => (theme::BODY, theme::BODY_LINE),
                };
                div()
                    .pt(px(if *level <= 2 { 6. } else { 2. }))
                    .text_size(px(size))
                    .line_height(px(line))
                    .child(self.inline(runs, Some(FontWeight::SEMIBOLD), veil, counter))
                    .into_any_element()
            }
            Block::CodeBlock { language, code } => {
                self.code_block(language.as_deref(), code, counter)
            }
            Block::BlockQuote { children } => div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .pl(px(14.))
                .border_l_2()
                .border_color(theme::border_strong())
                .text_color(theme::text_muted())
                .children(self.children(children, veil, counter))
                .into_any_element(),
            Block::List {
                ordered_start,
                items,
            } => {
                let count = items.len();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .children(items.iter().enumerate().map(|(ix, item)| {
                        let veil = veil.filter(|_| ix + 1 == count);
                        self.list_item(*ordered_start, ix, item, veil, counter)
                    }))
                    .into_any_element()
            }
            Block::Table {
                header,
                rows,
                align,
            } => self.table(header, rows, align, counter),
            Block::Rule => div()
                .my(px(4.))
                .h(px(1.))
                .w_full()
                .bg(theme::border_strong())
                .into_any_element(),
        }
    }

    fn children(
        &self,
        blocks: &[Block],
        veil: Option<&Veil>,
        counter: &mut usize,
    ) -> Vec<AnyElement> {
        let count = blocks.len();
        blocks
            .iter()
            .enumerate()
            .map(|(ix, block)| self.block(block, veil.filter(|_| ix + 1 == count), counter))
            .collect()
    }

    fn list_item(
        &self,
        ordered_start: Option<u64>,
        ix: usize,
        item: &[Block],
        veil: Option<&Veil>,
        counter: &mut usize,
    ) -> AnyElement {
        // A task item carries its `[ ]` marker as the first run.
        let task = item.first().and_then(|block| match block {
            Block::Paragraph { runs } => runs.first().and_then(|run| run.style.task.clone()),
            _ => None,
        });
        let marker: AnyElement = match (&task, ordered_start) {
            (Some(task), _) => checkbox(task.checked).into_any_element(),
            (None, Some(start)) => div()
                .text_color(theme::text_faint())
                .child(format!("{}.", start + ix as u64))
                .into_any_element(),
            (None, None) => div()
                .text_color(theme::text_faint())
                .child("•")
                .into_any_element(),
        };
        let content = if task.is_some() {
            let mut blocks = item.to_vec();
            if let Some(Block::Paragraph { runs }) = blocks.first_mut() {
                runs.retain(|run| run.style.task.is_none());
                if let Some(first) = runs.first_mut() {
                    first.text = first.text.trim_start().to_owned();
                }
            }
            self.children(&blocks, veil, counter)
        } else {
            self.children(item, veil, counter)
        };
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .justify_end()
                    .w(px(18.))
                    .h(px(theme::BODY_LINE))
                    .items_center()
                    .child(marker),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .flex_1()
                    .min_w_0()
                    .children(content),
            )
            .into_any_element()
    }

    fn code_block(&self, language: Option<&str>, code: &str, counter: &mut usize) -> AnyElement {
        let id = self.next_id(counter);
        let code = code.strip_suffix('\n').unwrap_or(code);
        div()
            .flex()
            .flex_col()
            .w_full()
            .rounded(px(10.))
            .border_1()
            .border_color(theme::border())
            .bg(theme::surface_card())
            .overflow_hidden()
            .child(
                div()
                    .px(px(12.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme::border())
                    .text_size(px(11.5))
                    .font_family(theme::MONO)
                    .text_color(theme::text_faint())
                    .child(language.unwrap_or("text").to_owned()),
            )
            .child(
                div()
                    .id(id)
                    .overflow_x_scroll()
                    .px(px(12.))
                    .py(px(10.))
                    .font_family(theme::MONO)
                    .text_size(px(theme::CODE))
                    .line_height(px(theme::CODE_LINE))
                    .text_color(theme::text())
                    .child(div().flex().flex_col().children(code.lines().map(|line| {
                        div().whitespace_nowrap().h(px(theme::CODE_LINE)).child(
                            if line.is_empty() {
                                " ".to_owned()
                            } else {
                                line.to_owned()
                            },
                        )
                    }))),
            )
            .into_any_element()
    }

    fn table(
        &self,
        header: &[Vec<InlineRun>],
        rows: &[Vec<Vec<InlineRun>>],
        align: &[TableAlign],
        counter: &mut usize,
    ) -> AnyElement {
        let mut row_elements: Vec<AnyElement> = Vec::new();
        let cell = |this: &Self,
                    runs: &[InlineRun],
                    col: usize,
                    weight: Option<FontWeight>,
                    counter: &mut usize| {
            let justify = align.get(col).copied().unwrap_or_default();
            let content = this.inline(runs, weight, None, counter);
            table_cell(content, justify)
        };
        row_elements.push(
            div()
                .flex()
                .flex_row()
                .bg(theme::input_bg())
                .text_color(theme::text_muted())
                .children(
                    header
                        .iter()
                        .enumerate()
                        .map(|(col, runs)| cell(self, runs, col, Some(FontWeight::MEDIUM), counter))
                        .collect::<Vec<_>>(),
                )
                .into_any_element(),
        );
        for row in rows {
            row_elements.push(
                div()
                    .flex()
                    .flex_row()
                    .border_t_1()
                    .border_color(theme::border())
                    .children(
                        row.iter()
                            .enumerate()
                            .map(|(col, runs)| cell(self, runs, col, None, counter))
                            .collect::<Vec<_>>(),
                    )
                    .into_any_element(),
            );
        }
        div()
            .flex()
            .flex_col()
            .w_full()
            .rounded(px(10.))
            .border_1()
            .border_color(theme::border())
            .overflow_hidden()
            .children(row_elements)
            .into_any_element()
    }

    fn inline(
        &self,
        runs: &[InlineRun],
        weight: Option<FontWeight>,
        veil: Option<&Veil>,
        counter: &mut usize,
    ) -> AnyElement {
        let mut text = String::new();
        let mut text_runs: Vec<TextRun> = Vec::new();
        let mut links: Vec<(Range<usize>, String)> = Vec::new();
        for run in runs {
            let style = &run.style;
            if style.task.is_some() {
                continue;
            }
            let shown = style.file_label.as_deref().unwrap_or(&run.text);
            let shown = match &style.image {
                Some(image) if shown.is_empty() => image.alt.as_str(),
                _ => shown,
            };
            if shown.is_empty() {
                continue;
            }
            let start = text.len();
            text.push_str(shown);
            let base_weight = weight.unwrap_or(FontWeight::NORMAL);
            let font = if style.code {
                theme::mono(FontWeight::NORMAL)
            } else {
                theme::sans(
                    if style.bold {
                        FontWeight::SEMIBOLD
                    } else {
                        base_weight
                    },
                    style.italic,
                )
            };
            let link = style.link.clone();
            let color = if link.is_some() {
                theme::accent()
            } else if style.code {
                theme::accent()
            } else {
                self.color
            };
            text_runs.push(TextRun {
                len: shown.len(),
                font,
                color,
                background_color: style.code.then(theme::accent_wash),
                underline: link.as_ref().map(|_| UnderlineStyle {
                    color: Some(theme::accent().opacity(0.4)),
                    thickness: px(1.),
                    wavy: false,
                }),
                strikethrough: style.strikethrough.then(|| StrikethroughStyle {
                    color: Some(self.color),
                    thickness: px(1.),
                }),
            });
            if let Some(link) = link {
                links.push((start..text.len(), link));
            }
        }
        if text.is_empty() {
            return div().into_any_element();
        }
        if let Some(veil) = veil.filter(|veil| !veil.is_empty()) {
            apply_veil(&text, &mut text_runs, veil);
        }
        let id = self.next_id(counter);
        let styled = StyledText::new(text).with_runs(text_runs);
        if links.is_empty() {
            return styled.into_any_element();
        }
        let ranges = links.iter().map(|(range, _)| range.clone()).collect();
        let urls: Vec<String> = links.into_iter().map(|(_, url)| url).collect();
        InteractiveText::new(id, styled)
            .on_click(ranges, move |ix, _window: &mut Window, cx: &mut App| {
                if let Some(url) = urls.get(ix)
                    && (url.starts_with("https://") || url.starts_with("http://"))
                {
                    cx.open_url(url);
                }
            })
            .into_any_element()
    }
}

fn table_cell(content: AnyElement, align: TableAlign) -> gpui::Div {
    // Block layout measures the text against the column's content width.
    // A flex row lets the text keep its intrinsic minimum and overflow.
    let cell = div().flex_1().min_w_0().px(px(12.)).py(px(7.));
    match align {
        TableAlign::Left => cell.text_left(),
        TableAlign::Center => cell.text_center(),
        TableAlign::Right => cell.text_right(),
    }
    .child(content)
}

fn checkbox(checked: bool) -> impl IntoElement {
    let base = div()
        .size(px(14.))
        .rounded(px(4.))
        .flex()
        .items_center()
        .justify_center();
    if checked {
        base.bg(theme::accent()).child(
            gpui::svg()
                .path("icons/check.svg")
                .size(px(11.))
                .text_color(theme::on_solid()),
        )
    } else {
        base.border_1().border_color(theme::border_strong())
    }
}

/// Multiplies each fading chunk's alpha into the last characters of `text`.
fn apply_veil(text: &str, runs: &mut Vec<TextRun>, veil: &Veil) {
    // Byte offsets of the last N characters, newest chunk at the very end.
    let char_starts: Vec<usize> = text.char_indices().map(|(ix, _)| ix).collect();
    let total = char_starts.len();
    let mut end_char = total;
    let mut spans: Vec<(Range<usize>, f32)> = Vec::new();
    for (chars, alpha) in veil.tail.iter().rev() {
        if end_char == 0 {
            break;
        }
        let start_char = end_char.saturating_sub(*chars);
        let start = char_starts[start_char];
        let end = if end_char == total {
            text.len()
        } else {
            char_starts[end_char]
        };
        spans.push((start..end, *alpha));
        end_char = start_char;
    }
    for (range, alpha) in spans {
        split_at(runs, range.start);
        split_at(runs, range.end);
        let mut offset = 0;
        for run in runs.iter_mut() {
            if offset >= range.start && offset + run.len <= range.end {
                run.color = run.color.opacity(alpha);
            }
            offset += run.len;
        }
    }
}

fn split_at(runs: &mut Vec<TextRun>, at: usize) {
    let mut offset = 0;
    for ix in 0..runs.len() {
        let len = runs[ix].len;
        if at > offset && at < offset + len {
            let mut tail = runs[ix].clone();
            tail.len = offset + len - at;
            runs[ix].len = at - offset;
            runs.insert(ix + 1, tail);
            return;
        }
        offset += len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AvailableSpace, TestAppContext, point, size};

    #[gpui::test]
    fn table_cells_wrap_at_phone_width(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        cx.simulate_resize(size(px(390.), px(844.)));
        cx.update(|window, cx| {
            let sources = [
                "A long table cell with several words that must wrap inside its column.",
                "https://example.com/averylongunbrokenpaththatmuststayinsideitscolumn",
                "averylongunbrokenidentifierthatmustwrapwithoutlosinganycharacters",
            ];
            let aligns = [TableAlign::Left, TableAlign::Center, TableAlign::Right];
            let mut layouts = Vec::new();
            let mut row = div().flex().w_full();
            for (ix, source) in sources.iter().enumerate() {
                let styled = StyledText::new(*source);
                layouts.push(styled.layout().clone());
                let content = if ix == 1 {
                    InteractiveText::new("table-link", styled)
                        .on_click(vec![0..source.len()], |_, _, _| {})
                        .into_any_element()
                } else {
                    styled.into_any_element()
                };
                row = row.child(table_cell(content, aligns[ix]));
            }
            // The card has the same side inset as a phone transcript.
            let mut card = div()
                .w(px(390.))
                .px(px(16.))
                .text_size(px(theme::BODY))
                .line_height(px(theme::BODY_LINE))
                .child(row)
                .into_any_element();
            card.prepaint_as_root(
                point(px(0.), px(0.)),
                size(
                    AvailableSpace::Definite(px(390.)),
                    AvailableSpace::MaxContent,
                ),
                window,
                cx,
            );
            let column_width = (390. - 32.) / 3.;
            for (ix, layout) in layouts.iter().enumerate() {
                let left = px(16. + ix as f32 * column_width + 12.);
                let right = left + px(column_width - 24.);
                assert!(layout.bounds().size.height > px(theme::BODY_LINE));
                assert_eq!(layout.wrapped_text().replace('\n', ""), sources[ix]);
                for (index, _) in sources[ix].char_indices() {
                    let position = layout.position_for_index(index).expect("visible character");
                    assert!(position.x >= left - px(1.), "column {ix}: {position:?}");
                    assert!(position.x <= right + px(1.), "column {ix}: {position:?}");
                }
            }
        });
    }
}
