//! Local tool, plugin, and delegation displays for the conversation previews.

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

pub fn tool_lines(call: &ToolCall, phase: u8, width: u16) -> Vec<Line<'static>> {
    let (label, accent) = match call.kind {
        ToolKind::Read => ("Read", t::ACCENT_SKILL),
        ToolKind::Search => ("Search", t::ACCENT_SKILL),
        ToolKind::Edit => ("Edit", t::ACCENT_SUCCESS),
        ToolKind::Run => ("Run", t::ACCENT_SUCCESS),
    };
    let (glyph, status_color) = match call.state {
        ToolState::Complete => ("◆", accent),
        ToolState::Running => (spinner(phase), accent),
        ToolState::Failed => ("×", t::DIFF_DELETE_FG),
    };
    let mut lines = vec![Line::from(vec![
        styled(format!(" {glyph} "), status_color),
        Span::styled(
            label,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        styled(call.input, t::TEXT_SECONDARY),
    ])];

    if call.kind == ToolKind::Edit && call.state == ToolState::Complete {
        let changes = coder_ui::components::diff::hunks(call.output);
        let added = changes
            .iter()
            .flatten()
            .filter(|line| line.change == coder_ui::components::diff::Change::Insert)
            .count();
        let removed = changes
            .iter()
            .flatten()
            .filter(|line| line.change == coder_ui::components::diff::Change::Delete)
            .count();
        lines[0].spans.extend([
            styled(format!(" +{added}"), t::DIFF_INSERT_FG),
            styled(format!(" -{removed}"), t::DIFF_DELETE_FG),
        ]);
        lines.extend(crate::rich_lines(coder_ui::components::diff::lines(
            call.output,
            call.input,
            usize::from(width),
        )));
        return lines;
    }

    match call.state {
        ToolState::Running => lines.push(Line::from(vec![
            styled("   ╰ ", t::GRAY_DIM),
            styled("Running", accent),
            styled(format!(" · {}", call.output), t::GRAY_BRIGHT),
        ])),
        ToolState::Failed => lines.push(Line::from(vec![
            styled("   ╰ ", t::GRAY_DIM),
            styled("Failed", t::DIFF_DELETE_FG),
            styled(format!(" · {}", call.output), t::DIFF_DELETE_FG),
        ])),
        ToolState::Complete => {
            for (index, output) in call.output.lines().enumerate() {
                let prefix = if index == 0 { "   ╰ " } else { "     " };
                lines.push(Line::from(vec![
                    styled(prefix, t::GRAY_DIM),
                    styled(output, t::GRAY_BRIGHT),
                ]));
            }
        }
    }
    lines
}

pub fn plugin_lines(call: &PluginCall, phase: u8) -> Vec<Line<'static>> {
    let (glyph, status_color) = match call.state {
        ToolState::Complete => ("◆", t::ACCENT_SKILL),
        ToolState::Running => (spinner(phase), t::ACCENT_SKILL),
        ToolState::Failed => ("×", t::DIFF_DELETE_FG),
    };
    let mut header = Line::from(vec![
        styled(format!(" {glyph} "), status_color),
        Span::styled(
            "Plugin",
            Style::default()
                .fg(t::ACCENT_SKILL)
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
    let mut result = vec![styled("   ╰ ", t::GRAY_DIM)];
    match call.state {
        ToolState::Complete => result.push(styled(call.output, t::GRAY_BRIGHT)),
        ToolState::Running => result.extend([
            styled("Running", t::ACCENT_SKILL),
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
    let mut header = vec![styled(" ◆ ", pulse(t::ACCENT_DELEGATE, phase))];
    if !narrow {
        header.extend([
            Span::styled(
                "Delegate",
                Style::default()
                    .fg(t::ACCENT_MODEL)
                    .add_modifier(Modifier::BOLD),
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
    let mut detail = vec![styled("   ╰ ", t::GRAY_DIM)];
    if task_width > 0 {
        detail.extend([
            styled(truncate(agent.task, task_width), t::TEXT_SECONDARY),
            styled(" · ", t::GRAY_DIM),
        ]);
    }
    detail.extend([
        styled("Running", t::ACCENT_MODEL),
        styled(tokens, t::GRAY_BRIGHT),
    ]);
    vec![Line::from(header), Line::from(detail)]
}

fn styled(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}

fn pulse(color: Color, phase: u8) -> Color {
    const LEVELS: [u16; 8] = [100, 85, 65, 45, 35, 55, 75, 95];
    let level = LEVELS[usize::from(phase) % LEVELS.len()];
    match color {
        Color::Rgb(red, green, blue) => Color::Rgb(
            (u16::from(red) * level / 100) as u8,
            (u16::from(green) * level / 100) as u8,
            (u16::from(blue) * level / 100) as u8,
        ),
        color => color,
    }
}
