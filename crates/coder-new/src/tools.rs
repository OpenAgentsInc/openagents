//! Local tool, plugin, and delegation displays for the conversation previews.

use code_highlight::grok::{ColorLevel, Palette};
use coder_terminal::components::diff;
use coder_terminal::selection::RowCopy;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::{agents::DemoAgent, theme as t, ui::truncate};

pub use coder_ui::demo::tools::{PluginCall, ToolCall, ToolKind, ToolState};

pub fn spinner(phase: u8) -> &'static str {
    const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    FRAMES[usize::from(phase) % FRAMES.len()]
}

pub(crate) fn outcome_header(running: bool, failed: bool, phase: u8) -> (&'static str, Color) {
    if running {
        (spinner(phase), t::GRAY)
    } else if failed {
        ("×", t::TOOL_FAILED)
    } else {
        ("●", t::ACCENT_SUCCESS)
    }
}

pub(crate) fn output_failed(output: &serde_json::Value) -> bool {
    output.get("error").is_some()
        || output
            .get("exit")
            .and_then(serde_json::Value::as_i64)
            .is_some_and(|exit| exit != 0)
        || output.get("timed_out").and_then(serde_json::Value::as_bool) == Some(true)
        || output.get("canceled").and_then(serde_json::Value::as_bool) == Some(true)
}

/// The keys a call names the file or folder it works on with, in the order
/// they are preferred.
const PATH_KEYS: [&str; 5] = ["path", "file_path", "notebook_path", "folder", "directory"];

/// Whether a flattened key is a top-level path key.
fn is_path_key(key: &str) -> bool {
    PATH_KEYS.contains(&key)
}

/// A value's `key: value` pairs, nested keys joined with a dot, at most two
/// levels deep, with a lone value named `value`.
fn fields(value: &serde_json::Value) -> Vec<(String, String)> {
    use serde_json::Value;
    fn walk(value: &Value, prefix: &str, depth: usize, result: &mut Vec<(String, String)>) {
        match value {
            Value::Object(object) if depth < 2 => {
                for (key, value) in object {
                    let key = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    walk(value, &key, depth + 1, result);
                }
            }
            Value::Null if prefix.is_empty() => {}
            value => {
                let text = match value {
                    Value::String(text) => text.replace('\n', " ↵ "),
                    _ => value.to_string(),
                };
                result.push((
                    if prefix.is_empty() {
                        "value".into()
                    } else {
                        prefix.into()
                    },
                    text,
                ));
            }
        }
    }
    let mut values = Vec::new();
    walk(value, "", 0, &mut values);
    values
}

/// A call's parameters, its path first: a nested `questions.refund` key is
/// never a path, so only a top-level key moves.
fn path_first(value: &serde_json::Value) -> Vec<(String, String)> {
    let mut values = fields(value);
    if let Some(index) = values.iter().position(|(key, _)| is_path_key(key)) {
        let path = values.remove(index);
        values.insert(0, path);
    }
    values
}

/// A call's parameters on one row, for the compact view: the path it works
/// on first and bare, then the rest as `key: value` joined with ` · `, all
/// together truncated to `width`. Empty when the call has none.
pub fn compact_params(value: &serde_json::Value, width: u16) -> String {
    let values = path_first(value);
    let path = values
        .first()
        .filter(|(key, _)| is_path_key(key))
        .map(|(_, value)| value.as_str());
    let rest = values
        .iter()
        .filter(|(key, _)| !is_path_key(key))
        .map(|(key, value)| format!("{key}: {value}"))
        .collect::<Vec<_>>()
        .join(" · ");
    let text = match (path, rest.is_empty()) {
        (Some(path), true) => path.to_owned(),
        (Some(path), false) => format!("{path} · {rest}"),
        (None, _) => rest,
    };
    if text.is_empty() {
        text
    } else {
        truncate(&text, width)
    }
}

/// Display bounded parameter rows without letting serialized objects run off screen.
/// A path the call names comes first, under the tool's name.
pub fn parameter_lines(value: &serde_json::Value, width: u16) -> Vec<Line<'static>> {
    let values = path_first(value);
    let omitted = values
        .len()
        .saturating_sub(if values.len() > 5 { 4 } else { 5 });
    let mut rows = Vec::new();
    for (key, value) in values.iter().take(if omitted > 0 { 4 } else { 5 }) {
        let key = truncate(key, width.saturating_sub(8));
        let available = width.saturating_sub(7 + key.width() as u16);
        rows.push(
            Line::from(vec![
                styled("  │  ", t::GRAY_DIM),
                styled(format!("{key}: "), t::TEXT_SECONDARY),
                styled(truncate(value, available), t::GRAY_BRIGHT),
            ])
            .style(Style::default().bg(t::BG_DARK)),
        );
    }
    if omitted > 0 {
        rows.push(
            Line::from(vec![
                styled("  │  ", t::GRAY_DIM),
                styled(
                    truncate(&format!("… {omitted} more fields"), width.saturating_sub(5)),
                    t::GRAY,
                ),
            ])
            .style(Style::default().bg(t::BG_DARK)),
        );
    }
    rows
}

pub fn tool_lines(call: &ToolCall, phase: u8, width: u16) -> Vec<Line<'static>> {
    let label = match call.kind {
        ToolKind::Read => "Read",
        ToolKind::Search => "Search",
        ToolKind::Edit => "Edit",
        ToolKind::Run => "Run",
    };
    let (glyph, accent) = outcome_header(
        call.state == ToolState::Running,
        call.state == ToolState::Failed,
        phase,
    );
    let mut lines = vec![Line::from(vec![
        styled(format!("{glyph} "), accent),
        Span::styled(
            label,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        styled(call.input, t::TEXT_SECONDARY),
    ])];

    if call.kind == ToolKind::Edit && call.state == ToolState::Complete {
        let changes = diff::hunks(call.output);
        let added = changes
            .iter()
            .flatten()
            .filter(|line| line.tag == diff::ChangeTag::Insert)
            .count();
        let removed = changes
            .iter()
            .flatten()
            .filter(|line| line.tag == diff::ChangeTag::Delete)
            .count();
        lines.push(edit_summary("Updated", call.input, added, removed, width));
        lines.extend(t::noir_lines(diff::lines(
            call.output,
            call.input,
            DIFF_LEAD,
            usize::from(width),
            Palette::Night,
            ColorLevel::TrueColor,
        )));
        return lines;
    }

    match call.state {
        ToolState::Running => lines.push(Line::from(vec![
            styled("  ⎿  ", t::GRAY_DIM),
            styled("Running", accent),
            styled(format!(" · {}", call.output), t::GRAY_BRIGHT),
        ])),
        ToolState::Failed => lines.push(Line::from(vec![
            styled("  ⎿  ", t::GRAY_DIM),
            styled("Failed", t::TOOL_FAILED),
            styled(format!(" · {}", call.output), t::TOOL_FAILED),
        ])),
        ToolState::Complete => {
            for (index, output) in call.output.lines().enumerate() {
                let prefix = if index == 0 { "  ⎿  " } else { "     " };
                lines.push(Line::from(vec![
                    styled(prefix, t::GRAY_DIM),
                    styled(output, t::GRAY_BRIGHT),
                ]));
            }
        }
    }
    lines
}

/// The cells before an edit's diff rows, so their numbers sit under the
/// summary's text.
const DIFF_LEAD: usize = 3;

/// An edit's summary under its header: `⎿  Updated <path> (+N -M)`.
fn edit_summary(verb: &str, path: &str, added: usize, removed: usize, width: u16) -> Line<'static> {
    let counts = format!(" (+{added} -{removed})");
    let room = width.saturating_sub(6 + verb.len() as u16 + counts.len() as u16);
    Line::from(vec![
        styled("  ⎿  ", t::GRAY_DIM),
        styled(format!("{verb} "), t::GRAY_BRIGHT),
        styled(truncate(path, room), t::TEXT_SECONDARY),
        styled(" (", t::GRAY_BRIGHT),
        styled(format!("+{added}"), t::DIFF_INSERT_FG),
        styled(" ", t::GRAY_BRIGHT),
        styled(format!("-{removed}"), t::DIFF_DELETE_FG),
        styled(")", t::GRAY_BRIGHT),
    ])
}

/// A built-in file tool's transcript rows (#11168): the tool and its path or
/// pattern, then a page count, the matches, or an edit's diff.
pub fn file_tool_lines(
    name: &str,
    input: &serde_json::Value,
    output: &serde_json::Value,
    running: bool,
    width: u16,
    phase: u8,
) -> Vec<Line<'static>> {
    file_tool_layout(name, input, output, running, width, phase).0
}

/// [`file_tool_lines`] with, row for row, what a copy takes from each: a
/// diff row copies its code alone, without the line number or the
/// `+`/`-` column, and its wrapped rows join back into one line.
pub(crate) fn file_tool_rows(
    name: &str,
    input: &serde_json::Value,
    output: &serde_json::Value,
    running: bool,
    width: u16,
    phase: u8,
) -> (Vec<Line<'static>>, Vec<RowCopy>) {
    let (lines, diff_rows) = file_tool_layout(name, input, output, running, width, phase);
    let mut copy: Vec<RowCopy> = lines.iter().map(plain_copy).collect();
    if let Some((rows, content)) = diff_rows {
        let gutter = DIFF_LEAD + diff::INDENT_CELLS..content;
        for index in rows.clone() {
            let text = line_text(&lines[index]);
            let row = &mut copy[index];
            if text.trim() == diff::SEPARATOR {
                row.omit = true;
                continue;
            }
            *row = RowCopy::lead(content as u16);
            let continued = index > rows.start
                && text
                    .get(gutter.clone())
                    .is_some_and(|cells| cells.trim().is_empty());
            if continued {
                row.joiner = Some(" ".into());
            }
        }
    }
    (lines, copy)
}

/// A drawn row's text, its spans joined.
pub(crate) fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

/// What a copy takes from a row drawn without a known layout: the text
/// after its leading marks (`●`, `⎿`, `│` and the spaces around them),
/// the spans that carry no letter or digit.
pub(crate) fn plain_copy(line: &Line<'_>) -> RowCopy {
    let mut lead = 0usize;
    for span in &line.spans {
        if span.content.chars().any(char::is_alphanumeric) {
            break;
        }
        lead += span.content.width();
    }
    RowCopy::lead(lead.min(usize::from(u16::MAX)) as u16)
}

/// The rows of a file tool, and for an edit, which of them are its diff
/// and the cell its code starts at.
fn file_tool_layout(
    name: &str,
    input: &serde_json::Value,
    output: &serde_json::Value,
    running: bool,
    width: u16,
    phase: u8,
) -> (Vec<Line<'static>>, Option<(std::ops::Range<usize>, usize)>) {
    let mut diff_rows = None;
    let text = |key: &str| {
        output
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let number = |key: &str| output.get(key).and_then(serde_json::Value::as_u64);
    let failed = output_failed(output);
    let (glyph, accent) = outcome_header(running, failed, phase);
    let subject = input
        .get("path")
        .filter(|_| !matches!(name, "Grep" | "Glob"))
        .or_else(|| input.get("pattern"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let path = output
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map_or(subject.clone(), str::to_owned);
    let header = vec![
        styled(format!("{glyph} "), accent),
        Span::styled(
            name.to_owned(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        styled(
            truncate(&path, width.saturating_sub(18 + name.width() as u16)),
            t::TEXT_SECONDARY,
        ),
    ];
    let mut lines = vec![Line::from(header)];
    let detail = |text: String, color: Color| {
        Line::from(vec![
            styled("  ⎿  ", t::GRAY_DIM),
            styled(truncate(&text, width.saturating_sub(5)), color),
        ])
    };
    if running {
        lines.push(detail("Running".into(), accent));
        return (lines, None);
    }
    if failed {
        lines.push(detail(text("error"), t::TOOL_FAILED));
        return (lines, None);
    }
    match name {
        "Edit" | "Write" => {
            let diff = text("diff");
            if diff.is_empty() {
                lines.push(detail("No changes".into(), t::GRAY_BRIGHT));
            } else {
                let added = number("added").unwrap_or(0) as usize;
                let removed = number("removed").unwrap_or(0) as usize;
                let verb = if name == "Write" { "Wrote" } else { "Updated" };
                lines.push(edit_summary(verb, &path, added, removed, width));
                let first = lines.len();
                lines.extend(t::noir_lines(diff::lines(
                    &diff,
                    &path,
                    DIFF_LEAD,
                    usize::from(width),
                    Palette::Night,
                    ColorLevel::TrueColor,
                )));
                diff_rows = Some((first..lines.len(), diff::content_column(&diff, DIFF_LEAD)));
                // The diff stops at its line limit; say what it left out.
                let shown = diff::hunks(&diff)
                    .iter()
                    .flatten()
                    .filter(|line| line.tag != diff::ChangeTag::Equal)
                    .count();
                let more = (added + removed).saturating_sub(shown);
                if more > 0 {
                    lines.push(Line::from(vec![
                        styled(" ".repeat(DIFF_LEAD + 2), t::GRAY_DIM),
                        styled(
                            format!("… {more} more {}", if more == 1 { "line" } else { "lines" }),
                            t::GRAY,
                        ),
                    ]));
                }
            }
        }
        "Read" => {
            let total = number("total_lines").unwrap_or(0);
            let shown = text("content").lines().count();
            lines.push(detail(
                if output.get("next_offset").is_some() {
                    format!("{shown} of {total} lines")
                } else {
                    format!("{total} lines")
                },
                t::GRAY_BRIGHT,
            ));
        }
        _ => {
            let (body, summary) = if name == "Grep" {
                let count = number("count").unwrap_or(0);
                let files = number("files").unwrap_or(0);
                (
                    text("matches"),
                    format!(
                        "{count} {} in {files} {}",
                        if count == 1 { "match" } else { "matches" },
                        if files == 1 { "file" } else { "files" }
                    ),
                )
            } else {
                let count = number("count").unwrap_or(0);
                (
                    text("files"),
                    format!("{count} {}", if count == 1 { "file" } else { "files" }),
                )
            };
            lines.push(detail(summary, t::GRAY_BRIGHT));
            if number("count").unwrap_or(0) > 0 {
                let rows: Vec<&str> = body.lines().collect();
                for row in rows.iter().take(5) {
                    lines.push(Line::from(vec![
                        styled("     ", t::GRAY_DIM),
                        styled(truncate(row, width.saturating_sub(5)), t::GRAY),
                    ]));
                }
                if rows.len() > 5 {
                    lines.push(Line::from(vec![
                        styled("     ", t::GRAY_DIM),
                        styled(format!("… {} more", rows.len() - 5), t::GRAY_DIM),
                    ]));
                }
            }
        }
    }
    (lines, diff_rows)
}

pub fn plugin_lines(call: &PluginCall, phase: u8) -> Vec<Line<'static>> {
    let (glyph, status_color) = outcome_header(
        call.state == ToolState::Running,
        call.state == ToolState::Failed,
        phase,
    );
    let mut header = Line::from(vec![
        styled(format!("{glyph} "), status_color),
        Span::styled(
            "Plugin",
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        styled(
            format!(" {}.{}", call.plugin, call.operation),
            t::TEXT_PRIMARY,
        ),
    ]);
    if !call.input.is_empty() {
        header
            .spans
            .push(styled(format!(" · {}", call.input), t::GRAY_BRIGHT));
    }
    let mut result = vec![styled("  ⎿  ", t::GRAY_DIM)];
    match call.state {
        ToolState::Complete => result.push(styled(call.output, t::GRAY_BRIGHT)),
        ToolState::Running => result.extend([
            styled("Running", t::GRAY),
            styled(format!(" · {}", call.output), t::GRAY_BRIGHT),
        ]),
        ToolState::Failed => result.extend([
            styled("Failed", t::TOOL_FAILED),
            styled(format!(" · {}", call.output), t::TOOL_FAILED),
        ]),
    }
    vec![header, Line::from(result)]
}

pub fn delegation_lines(agent: &DemoAgent, phase: u8, width: u16) -> Vec<Line<'static>> {
    let narrow = width < 32;
    let mut header = vec![styled("● ", outcome_header(true, false, phase).1)];
    if !narrow {
        header.extend([
            Span::styled(
                "Delegate",
                Style::default().fg(t::GRAY).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]);
    }
    header.push(styled(agent.name, t::TEXT_PRIMARY));
    let tokens = if narrow {
        format!(" · {}", agent.tokens)
    } else {
        format!(" · {} tokens", agent.tokens)
    };
    let task_width = width.saturating_sub(5 + 3 + 7 + tokens.width() as u16);
    let mut detail = vec![styled("  ⎿  ", t::GRAY_DIM)];
    if task_width > 0 {
        detail.extend([
            styled(truncate(agent.task, task_width), t::TEXT_SECONDARY),
            styled(" · ", t::GRAY_DIM),
        ]);
    }
    detail.extend([styled("Running", t::GRAY), styled(tokens, t::GRAY_BRIGHT)]);
    vec![Line::from(header), Line::from(detail)]
}

fn styled(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}
