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

/// A tool call's input on its header row: gray while it runs or after it
/// failed, like the call's name, and bright once it succeeded.
pub(crate) fn input_color(running: bool, failed: bool) -> Color {
    if running || failed {
        t::GRAY
    } else {
        t::TEXT_SECONDARY
    }
}

pub(crate) fn output_failed(output: &serde_json::Value) -> bool {
    atif::row::failed(output)
}

/// A call's parameters on one row, for the compact view: a command-line
/// tool's arguments as the command line it runs ("projects show
/// finances"), else the path it works on first and bare, then the rest as
/// `key: value` joined with ` · `, all together truncated to `width`.
/// Empty when the call has none. The words are [`atif::row::params`], the
/// same the account copy of the chat on openagents.com shows (#11543).
pub fn compact_params(value: &serde_json::Value, width: u16) -> String {
    let text = atif::row::params(value);
    if text.is_empty() {
        text
    } else {
        truncate(&text, width)
    }
}

/// Display bounded parameter rows without letting serialized objects run off screen.
/// A path the call names comes first, under the tool's name.
pub fn parameter_lines(value: &serde_json::Value, width: u16) -> Vec<Line<'static>> {
    let values = atif::row::path_first(value);
    let omitted = values
        .len()
        .saturating_sub(if values.len() > 5 { 4 } else { 5 });
    let mut rows = Vec::new();
    for (key, value) in values.iter().take(if omitted > 0 { 4 } else { 5 }) {
        let key = truncate(key, width.saturating_sub(8));
        let available = width.saturating_sub(7 + key.width() as u16);
        // A command shows as the command alone, without its `command: ` label.
        let label = if key == "command" {
            String::new()
        } else {
            format!("{key}: ")
        };
        rows.push(
            Line::from(vec![
                styled("  │  ", t::GRAY_DIM),
                styled(label, t::TEXT_SECONDARY),
                styled(truncate(value, available), t::TEXT_SECONDARY),
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
                    t::TEXT_SECONDARY,
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
        styled(
            call.input,
            input_color(
                call.state == ToolState::Running,
                call.state == ToolState::Failed,
            ),
        ),
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
        lines[0].spans.extend(edit_counts(added, removed));
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
            styled(format!(" · {}", call.output), t::TEXT_SECONDARY),
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
                    styled(output, t::TEXT_SECONDARY),
                ]));
            }
        }
    }
    lines
}

/// The cells before an edit's diff rows, so their numbers sit under the
/// header's text.
const DIFF_LEAD: usize = 3;

/// A finished edit's counts after its header path: ` (+N -M)`. The header
/// carries them, so no summary row repeats the path under it.
fn edit_counts(added: usize, removed: usize) -> Vec<Span<'static>> {
    vec![
        styled(" (", t::TEXT_SECONDARY),
        styled(format!("+{added}"), t::DIFF_INSERT_FG),
        styled(" ", t::TEXT_SECONDARY),
        styled(format!("-{removed}"), t::DIFF_DELETE_FG),
        styled(")", t::TEXT_SECONDARY),
    ]
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
        .or_else(|| input.get("file_path"))
        .or_else(|| input.get("filePath"))
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
                // Only Coder's own edit reports its diff; another harness's
                // edit (OpenCode, Claude Code) says nothing about it, which
                // is not the same as changing nothing.
                if output.get("diff").is_some() {
                    lines.push(detail("No changes".into(), t::TEXT_SECONDARY));
                }
            } else {
                let added = number("added").unwrap_or(0) as usize;
                let removed = number("removed").unwrap_or(0) as usize;
                lines[0].spans.extend(edit_counts(added, removed));
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
                            t::TEXT_SECONDARY,
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
                t::TEXT_SECONDARY,
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
            lines.push(detail(summary, t::TEXT_SECONDARY));
            if number("count").unwrap_or(0) > 0 {
                let rows: Vec<&str> = body.lines().collect();
                for row in rows.iter().take(5) {
                    lines.push(Line::from(vec![
                        styled("     ", t::GRAY_DIM),
                        styled(truncate(row, width.saturating_sub(5)), t::TEXT_SECONDARY),
                    ]));
                }
                if rows.len() > 5 {
                    lines.push(Line::from(vec![
                        styled("     ", t::GRAY_DIM),
                        styled(format!("… {} more", rows.len() - 5), t::TEXT_SECONDARY),
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
            .push(styled(format!(" · {}", call.input), t::TEXT_SECONDARY));
    }
    let mut result = vec![styled("  ⎿  ", t::GRAY_DIM)];
    match call.state {
        ToolState::Complete => result.push(styled(call.output, t::TEXT_SECONDARY)),
        ToolState::Running => result.extend([
            styled("Running", t::GRAY),
            styled(format!(" · {}", call.output), t::TEXT_SECONDARY),
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
    detail.extend([
        styled("Running", t::GRAY),
        styled(tokens, t::TEXT_SECONDARY),
    ]);
    vec![Line::from(header), Line::from(detail)]
}

fn styled(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{compact_params, parameter_lines};

    #[test]
    fn a_command_shows_bare_and_other_parameters_keep_their_label() {
        let input = json!({"command": "cargo test", "timeout": 5});
        assert_eq!(compact_params(&input, 80), "cargo test · timeout: 5");
        let rows: Vec<String> = parameter_lines(&input, 80)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].trim_end(), "  │  cargo test");
        assert_eq!(rows[1].trim_end(), "  │  timeout: 5");
    }
    #[test]
    fn a_command_line_tools_arguments_read_as_its_command_line() {
        let input = serde_json::json!({"arguments": ["projects", "show", "finances"]});
        assert_eq!(compact_params(&input, 80), "projects show finances");
        let quoted = serde_json::json!({"args": ["issue", "create", "--title", "Fix the rail"]});
        assert_eq!(
            compact_params(&quoted, 80),
            "issue create --title 'Fix the rail'"
        );
        // Anything else keeps the key: value form.
        let other = serde_json::json!({"arguments": ["a"], "cwd": "x"});
        assert!(compact_params(&other, 80).contains("cwd: x"));
    }
    #[test]
    fn a_calls_input_is_gray_until_it_succeeds() {
        use crate::theme as t;
        assert_eq!(super::input_color(true, false), t::GRAY);
        assert_eq!(super::input_color(false, true), t::GRAY);
        assert_eq!(super::input_color(false, false), t::TEXT_SECONDARY);
    }
}
