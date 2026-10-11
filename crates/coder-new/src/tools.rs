//! Local tool, plugin, and delegation displays for the conversation previews.

use code_highlight::grok::{ColorLevel, Palette};
use coder_terminal::components::diff;
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
        ("×", t::DIFF_DELETE_FG)
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

/// Display bounded parameter rows without letting serialized objects run off screen.
pub fn parameter_lines(value: &serde_json::Value, width: u16) -> Vec<Line<'static>> {
    use serde_json::Value;
    fn fields(value: &Value, prefix: &str, depth: usize, result: &mut Vec<(String, String)>) {
        match value {
            Value::Object(object) if depth < 2 => {
                for (key, value) in object {
                    let key = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    fields(value, &key, depth + 1, result);
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
    fields(value, "", 0, &mut values);
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
        lines[0].spans.extend([
            styled(format!(" +{added}"), t::DIFF_INSERT_FG),
            styled(format!(" -{removed}"), t::DIFF_DELETE_FG),
        ]);
        lines.extend(t::noir_lines(diff::lines(
            call.output,
            call.input,
            0,
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
            styled("Failed", t::DIFF_DELETE_FG),
            styled(format!(" · {}", call.output), t::DIFF_DELETE_FG),
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
    let text = |key: &str| {
        output
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let number = |key: &str| output.get(key).and_then(serde_json::Value::as_u64);
    let kind = if matches!(name, "Edit" | "Write") {
        "edit"
    } else {
        "read"
    };
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
    let mut header = vec![
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
    if kind == "edit" && !running && !failed {
        header.extend([
            styled(
                format!(" +{}", number("added").unwrap_or(0)),
                t::DIFF_INSERT_FG,
            ),
            styled(
                format!(" -{}", number("removed").unwrap_or(0)),
                t::DIFF_DELETE_FG,
            ),
        ]);
    }
    let mut lines = vec![Line::from(header)];
    let detail = |text: String, color: Color| {
        Line::from(vec![
            styled("  ⎿  ", t::GRAY_DIM),
            styled(truncate(&text, width.saturating_sub(5)), color),
        ])
    };
    if running {
        lines.push(detail("Running".into(), accent));
        return lines;
    }
    if failed {
        lines.push(detail(text("error"), t::DIFF_DELETE_FG));
        return lines;
    }
    match name {
        "Edit" | "Write" => {
            let diff = text("diff");
            if diff.is_empty() {
                lines.push(detail("No changes".into(), t::GRAY_BRIGHT));
            } else {
                lines.extend(t::noir_lines(diff::lines(
                    &diff,
                    &path,
                    0,
                    usize::from(width),
                    Palette::Night,
                    ColorLevel::TrueColor,
                )));
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
    lines
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
            styled("Failed", t::DIFF_DELETE_FG),
            styled(format!(" · {}", call.output), t::DIFF_DELETE_FG),
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
