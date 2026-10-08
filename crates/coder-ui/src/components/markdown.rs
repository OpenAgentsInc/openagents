//! Selectable Markdown rows, reimplemented from the public terminal renderer.
//! Boxed tables and quote/list geometry retain the grok-build Apache-2.0
//! attribution in `coder-terminal/src/markdown.rs` and this crate's NOTICE.

use crate::{
    components::{run, syntax, wrap},
    source_theme as t,
};
use rust_native::{
    markdown::{Block, Span},
    style::Color,
    view::RichRun,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub type Lines = Vec<Vec<RichRun>>;

pub fn lines(source: &str, width: usize) -> Lines {
    let source = display_source(source);
    let mut out = Vec::new();
    blocks(
        &rust_native::markdown::parse(&source),
        "",
        width.max(1),
        &mut out,
    );
    out
}

// The terminal projects images exactly like inert links, including the URL.
fn display_source(source: &str) -> String {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    let mut edits = Vec::new();
    for (event, range) in Parser::new_ext(
        source,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    )
    .into_offset_iter()
    {
        match event {
            Event::Html(text) | Event::InlineHtml(text) => {
                let mut escaped = String::new();
                for ch in text.chars() {
                    if "\\`*{}_[]()#+-.!<>".contains(ch) {
                        escaped.push('\\');
                    }
                    escaped.push(ch);
                }
                edits.push((range, escaped));
            }
            Event::Start(Tag::Image { .. }) => {
                let text = &source[range.clone()];
                if let Some(link) = text.strip_prefix('!') {
                    edits.push((range, link.into()));
                }
            }
            _ => {}
        }
    }
    let mut out = String::new();
    let mut cursor = 0;
    for (range, text) in edits {
        if range.start < cursor {
            continue;
        }
        out.push_str(&source[cursor..range.start]);
        out.push_str(&text);
        cursor = range.end;
    }
    out.push_str(&source[cursor..]);
    out
}

fn spans(spans: &[Span], color: Color, heading: bool) -> Vec<RichRun> {
    let mut out = Vec::new();
    for (i, span) in spans.iter().enumerate() {
        let mut piece = run(
            &span.text,
            if span.code || span.link.is_some() {
                t::MD_CODE
            } else {
                color
            },
        );
        piece.bold = heading || span.bold || span.code;
        piece.italic = span.italic;
        piece.strike = span.strike;
        piece.underline = span.link.is_some();
        out.push(piece);
        if let Some(url) = &span.link {
            if spans.get(i + 1).and_then(|next| next.link.as_ref()) != Some(url) {
                out.push(run(format!(" ({url})"), t::GRAY));
            }
        }
    }
    out
}

fn prefix(value: &str) -> Vec<RichRun> {
    value
        .chars()
        .map(|ch| {
            let mut piece = run(ch.to_string(), t::GRAY);
            piece.dim = ch == '│';
            piece
        })
        .collect()
}

fn paragraph(mut value: Vec<RichRun>, lead: &str, width: usize, out: &mut Lines) {
    let mut first = prefix(lead);
    first.append(&mut value);
    let hang = lead.width().min(width.saturating_sub(1));
    for (i, mut row) in wrap(&first, width.saturating_sub(hang))
        .into_iter()
        .enumerate()
    {
        if i > 0 && hang > 0 {
            row.insert(0, run(" ".repeat(hang), t::TEXT_SECONDARY));
        }
        out.push(row);
    }
}

fn blocks(items: &[Block], lead: &str, width: usize, out: &mut Lines) {
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(prefix(lead.trim_end()));
        }
        block(item, lead, width, out);
    }
}

fn block(item: &Block, lead: &str, width: usize, out: &mut Lines) {
    match item {
        Block::Paragraph { spans: value } => {
            paragraph(spans(value, t::TEXT_SECONDARY, false), lead, width, out)
        }
        Block::Heading {
            level,
            spans: value,
        } => {
            let color = [
                t::ACCENT_SKILL,
                t::ACCENT_SKILL,
                t::ACCENT_DELEGATE,
                t::GRAY_BRIGHT,
                t::GRAY,
                t::GRAY_DIM,
            ][usize::from((*level).clamp(1, 6) - 1)];
            paragraph(spans(value, color, true), lead, width, out);
        }
        Block::Code { language, text } => {
            let rows = syntax::lines(text, language.as_deref().unwrap_or("unknown"));
            for source_row in rows {
                let mut source_row = source_row;
                source_row.insert(0, run(lead, t::GRAY));
                for mut row in wrap(&source_row, width) {
                    let used = row.iter().map(|r| r.text.width()).sum::<usize>();
                    for piece in &mut row {
                        piece.background = Some(t::BG_DARK);
                    }
                    let mut pad = run(" ".repeat(width.saturating_sub(used)), t::TEXT_SECONDARY);
                    pad.background = Some(t::BG_DARK);
                    row.push(pad);
                    out.push(row);
                }
            }
        }
        Block::Quote { blocks: value } => blocks(value, &format!("{lead}│ "), width, out),
        Block::List {
            ordered,
            start,
            items,
        } => {
            for (i, item) in items.iter().enumerate() {
                let marker = match item.checked {
                    Some(true) => "[x] ".into(),
                    Some(false) => "[ ] ".into(),
                    None if *ordered => format!("{}. ", start + i as u64),
                    None => "• ".into(),
                };
                let padding = " ".repeat(marker.width());
                for (j, part) in item.blocks.iter().enumerate() {
                    if j > 0 && !matches!(part, Block::List { .. }) {
                        out.push(Vec::new());
                    }
                    block(
                        part,
                        &format!("{lead}{}", if j == 0 { &marker } else { &padding }),
                        width,
                        out,
                    );
                }
            }
        }
        Block::Table { header, rows, .. } => table(header, rows, lead, width, out),
        Block::Rule => out.push(vec![run(format!("{lead}───"), t::GRAY)]),
    }
}

fn table(
    header: &[Vec<Span>],
    rows: &[Vec<Vec<Span>>],
    lead: &str,
    viewport: usize,
    out: &mut Lines,
) {
    let columns = std::iter::once(header.len())
        .chain(rows.iter().map(Vec::len))
        .max()
        .unwrap_or(0);
    if columns == 0 {
        return;
    }
    let mut widths = vec![0usize; columns];
    let mut words = widths.clone();
    let mut floors = widths.clone();
    for row in std::iter::once(header).chain(rows.iter().map(Vec::as_slice)) {
        for (i, cell) in row.iter().enumerate() {
            let value = spans(cell, t::TEXT_SECONDARY, false)
                .into_iter()
                .map(|r| r.text)
                .collect::<String>();
            widths[i] = widths[i].max(value.split('\n').map(str::width).max().unwrap_or(0));
            words[i] = words[i].max(value.split_whitespace().map(str::width).max().unwrap_or(0));
            floors[i] = floors[i].max(
                value
                    .graphemes(true)
                    .filter(|g| *g != "\n")
                    .map(|g| g.width().max(1))
                    .max()
                    .unwrap_or(0),
            );
            words[i] = words[i].max(floors[i]);
            widths[i] = widths[i].max(words[i]);
        }
    }
    let available = viewport.saturating_sub(lead.width());
    let overhead = columns * 3 + 1;
    let budget = available.saturating_sub(overhead);
    if available < overhead || floors.iter().sum::<usize>() > budget {
        let data: Vec<_> = if rows.is_empty() {
            vec![header.to_vec()]
        } else {
            rows.to_vec()
        };
        for (r, cells) in data.iter().enumerate() {
            if r > 0 {
                out.push(Vec::new());
            }
            for i in 0..columns {
                let mut value = spans(
                    header.get(i).map(Vec::as_slice).unwrap_or(&[]),
                    t::TEXT_PRIMARY,
                    true,
                );
                if !value.is_empty() {
                    value.push(run(": ", t::GRAY));
                }
                value.extend(spans(
                    cells.get(i).map(Vec::as_slice).unwrap_or(&[]),
                    t::TEXT_SECONDARY,
                    false,
                ));
                paragraph(value, lead, viewport, out);
            }
        }
        return;
    }
    if widths.iter().sum::<usize>() > budget {
        let targets = if words.iter().sum::<usize>() <= budget {
            widths.clone()
        } else {
            words.clone()
        };
        widths = if words.iter().sum::<usize>() <= budget {
            words
        } else {
            floors
        };
        let extra = budget.saturating_sub(widths.iter().sum());
        let wants: Vec<_> = targets
            .iter()
            .zip(&widths)
            .map(|(target, base)| target.saturating_sub(*base))
            .collect();
        let total = wants.iter().sum::<usize>();
        if total > 0 {
            for (w, want) in widths.iter_mut().zip(&wants) {
                *w += ((*want as u128 * extra as u128) / total as u128) as usize;
            }
            let mut remaining = budget.saturating_sub(widths.iter().sum());
            let mut indices = (0..columns).collect::<Vec<_>>();
            indices.sort_by_key(|&i| std::cmp::Reverse(targets[i].saturating_sub(widths[i])));
            for i in indices {
                if remaining > 0 && widths[i] < targets[i] {
                    widths[i] += 1;
                    remaining -= 1;
                }
            }
        }
    }
    let border = |left, mid, right| {
        let mut value = lead.to_owned();
        value.push(left);
        for (i, w) in widths.iter().enumerate() {
            value.push_str(&"─".repeat(w + 2));
            value.push(if i + 1 < columns { mid } else { right });
        }
        let mut r = run(value, t::GRAY);
        r.dim = true;
        vec![r]
    };
    let cell_rows = |cells: &[Vec<Span>], is_header: bool| {
        let wrapped = widths
            .iter()
            .enumerate()
            .map(|(i, w)| {
                wrap(
                    &spans(
                        cells.get(i).map(Vec::as_slice).unwrap_or(&[]),
                        if is_header {
                            t::TEXT_PRIMARY
                        } else {
                            t::TEXT_SECONDARY
                        },
                        is_header,
                    ),
                    *w,
                )
            })
            .collect::<Vec<_>>();
        (0..wrapped.iter().map(Vec::len).max().unwrap_or(1))
            .map(|r| {
                let mut row = prefix(lead);
                let mut bar = run("│", t::GRAY);
                bar.dim = true;
                row.push(bar.clone());
                for (i, w) in widths.iter().enumerate() {
                    row.push(run(" ", t::TEXT_SECONDARY));
                    let value = wrapped[i].get(r).cloned().unwrap_or_default();
                    let used = value.iter().map(|p| p.text.width()).sum::<usize>();
                    row.extend(value);
                    row.push(run(
                        " ".repeat(w.saturating_sub(used) + 1),
                        t::TEXT_SECONDARY,
                    ));
                    row.push(bar.clone());
                }
                row
            })
            .collect::<Lines>()
    };
    out.push(border('┌', '┬', '┐'));
    out.extend(cell_rows(header, true));
    out.push(border('├', '┼', '┤'));
    for (i, row) in rows.iter().enumerate() {
        out.extend(cell_rows(row, false));
        if i + 1 < rows.len() {
            out.push(border('├', '┼', '┤'));
        }
    }
    out.push(border('└', '┴', '┘'));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_literal_markup_and_table_content_at_small_widths() {
        let source = "<script>literal</script>\n\n| Name | Value |\n| --- | --- |\n| A | 日本語 |";
        let text = lines(source, 8)
            .into_iter()
            .flatten()
            .map(|r| r.text)
            .collect::<String>();
        assert!(text.contains("<script>literal</script>"));
        assert!(text.contains("日本語"));
        assert!(!text.contains("┌"));
        let raw = lines("<div>Literal HTML</div>", 110);
        assert!(
            raw.iter()
                .flatten()
                .all(|r| r.foreground == Some(t::TEXT_SECONDARY) && !r.bold)
        );
        let code = lines("`<div>Code</div>`", 110);
        assert!(
            code.iter()
                .flatten()
                .any(|r| r.foreground == Some(t::MD_CODE) && r.bold)
        );
    }
    #[test]
    fn inline_modifiers_and_destinations_remain_typed() {
        let values=lines("**Bold** _italic_ ~~gone~~ `code` [link](https://example.com) ![alt](https://example.com/a.png)",110).into_iter().flatten().collect::<Vec<_>>();
        assert!(values.iter().any(|r| r.bold && r.text == "Bold"));
        assert!(values.iter().any(|r| r.italic));
        assert!(values.iter().any(|r| r.strike));
        assert!(values.iter().any(|r| r.text.contains("/a.png")));
    }
}
