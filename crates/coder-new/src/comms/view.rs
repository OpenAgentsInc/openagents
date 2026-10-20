//! How the channel and the swarm show in a chat.
//!
//! A channel record becomes a finished `Channel` tool entry in each child
//! chat it concerns ([`event`]); the swarm's own chat holds one `Swarm`
//! entry it keeps current, a row per member with its stage, cost and
//! outcome ([`swarm_lines`]).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::{Value, json};
use unicode_width::UnicodeWidthStr;

use super::{ALL, Kind, Record};
use crate::bundled_runtime::RuntimeEvent;
use crate::theme as t;

/// The tool entry a channel record is drawn as.
pub const CHANNEL: &str = "Channel";

/// The tool entry the swarm view is drawn as.
pub const SWARM: &str = "Swarm";

/// The chat event that shows `record`.
#[must_use]
pub fn event(record: &Record) -> RuntimeEvent {
    RuntimeEvent::Tool {
        name: CHANNEL.into(),
        input: json!({
            "from": record.from,
            "to": record.to,
            "kind": record.kind.as_str(),
            "at_ms": record.at_ms,
        }),
        output: json!({"text": record.text}),
        running: false,
    }
}

fn span(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}

/// Words of `text` in lines no wider than `width`.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(10);
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if !line.is_empty() && line.width() + 1 + word.width() > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        lines.push(line);
    }
    lines
}

/// A `Channel` entry's rows: who wrote to whom, then the words.
#[must_use]
pub fn channel_lines(input: &Value, output: &Value, width: u16) -> Vec<Line<'static>> {
    let from = input["from"].as_str().unwrap_or_default();
    let to = input["to"].as_str().unwrap_or(ALL);
    let kind = input["kind"].as_str().unwrap_or("message");
    let label = match kind {
        "overlap" => "Overlap",
        "agreement" => "Agreed",
        "lease" => "Lease",
        "intent" => "Intent",
        "finish" => "Finished",
        _ => "Channel",
    };
    let color = match kind {
        "overlap" => t::COMMAND,
        "agreement" => t::ACCENT_SUCCESS,
        _ => t::ACCENT_DELEGATE,
    };
    let who = if kind == "message" {
        if to == ALL {
            format!(" {from} to everyone")
        } else {
            format!(" {from} to {to}")
        }
    } else if from == super::SYSTEM {
        String::new()
    } else {
        format!(" {from}")
    };
    let mut lines = vec![Line::from(vec![
        span("◆ ", color),
        Span::styled(
            label,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        span(
            crate::ui::truncate(&who, width.saturating_sub(12)),
            t::TEXT_SECONDARY,
        ),
    ])];
    let text = output["text"].as_str().unwrap_or_default();
    for (index, row) in wrap(text, usize::from(width.saturating_sub(6)))
        .into_iter()
        .enumerate()
    {
        let lead = if index == 0 { "  ⎿  " } else { "     " };
        lines.push(Line::from(vec![
            span(lead, t::GRAY_DIM),
            span(row, t::TEXT_PRIMARY),
        ]));
    }
    lines
}

/// The swarm view's rows: a heading with the totals, then a row per member.
#[must_use]
pub fn swarm_lines(output: &Value, running: bool, width: u16, phase: u8) -> Vec<Line<'static>> {
    let members = output["members"].as_array().cloned().unwrap_or_default();
    let busy = members
        .iter()
        .filter(|m| m["outcome"].is_null() && m["stage"] != "Waiting")
        .count();
    let done = members.iter().filter(|m| !m["outcome"].is_null()).count();
    let spent = output["spent_usd"].as_f64().unwrap_or(0.0);
    let (glyph, color) = crate::tools::outcome_header(running, false, phase);
    let mut heading = format!(" {} solvers", members.len());
    if running {
        heading.push_str(&format!(" · {busy} working · {done} done"));
    } else {
        heading.push_str(&format!(" · {done} done"));
    }
    heading.push_str(&format!(" · ${spent:.2}"));
    let mut lines = vec![Line::from(vec![
        span(format!("{glyph} "), color),
        Span::styled(
            "Solver swarm",
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        span(
            crate::ui::truncate(&heading, width.saturating_sub(16)),
            t::TEXT_SECONDARY,
        ),
    ])];
    for member in &members {
        let name = member["name"].as_str().unwrap_or_default();
        let stage = member["stage"].as_str().unwrap_or_default();
        let cost = member["cost_usd"]
            .as_f64()
            .map_or_else(|| "    —".to_owned(), |cost| format!("${cost:>4.2}"));
        let outcome = match member["outcome"].as_str() {
            None => String::new(),
            Some(outcome) => {
                let mut text = super::swarm::outcome_words(outcome);
                if let Some(url) = member["pull_request"].as_str() {
                    text.push_str(&format!(" · {url}"));
                } else if let Some(reason) = member["reason"].as_str() {
                    text.push_str(&format!(" · {reason}"));
                }
                text
            }
        };
        let row = format!("{name:<14} {stage:<18} {cost}  {outcome}");
        let row_color = match member["outcome"].as_str() {
            Some("passed") => t::ACCENT_SUCCESS,
            Some(_) => t::GRAY_BRIGHT,
            None => t::TEXT_PRIMARY,
        };
        lines.push(Line::from(vec![
            span("  ", t::GRAY_DIM),
            span(
                crate::ui::truncate(&row, width.saturating_sub(2)),
                row_color,
            ),
        ]));
    }
    if let Some(summary) = output["summary"].as_str() {
        for (index, row) in wrap(summary, usize::from(width.saturating_sub(6)))
            .into_iter()
            .enumerate()
        {
            let lead = if index == 0 { "  ⎿  " } else { "     " };
            lines.push(Line::from(vec![
                span(lead, t::GRAY_DIM),
                span(row, t::TEXT_PRIMARY),
            ]));
        }
    }
    lines
}

/// What a channel record says in one plain line, for text-only places.
#[must_use]
pub fn plain(record: &Record) -> String {
    match record.kind {
        Kind::Message if record.to == ALL => {
            format!("{} to everyone: {}", record.from, record.text)
        }
        Kind::Message => format!("{} to {}: {}", record.from, record.to, record.text),
        _ => record.text.clone(),
    }
}
