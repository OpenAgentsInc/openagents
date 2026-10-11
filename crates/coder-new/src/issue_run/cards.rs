//! How decision cards and the summary card draw.
//!
//! A decision card is a tool-call card of its own style: a hollow `◇`
//! glyph, the step's kind as its label (Stage, Model, Ranked, Briefing,
//! Agent), and a rail down its left side. A model card shows its question,
//! each option with a probability bar, the answer, the door and model that
//! answered, the latency and the cost. The summary card closes the run
//! with time, tokens, dollars, the files opened outside the briefing, the
//! check results, the comparison with a closed issue's real fix, and the
//! diff, drawn with Coder's diff renderer.

use code_highlight::grok::{ColorLevel, Palette};
use coder_terminal::components::diff;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use serde_json::Value;

use crate::{theme as t, ui::truncate};

fn styled(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}

const RAIL: &str = "  │  ";

fn rail(spans: Vec<Span<'static>>) -> Line<'static> {
    let mut all = vec![styled(RAIL, t::GRAY_DIM)];
    all.extend(spans);
    Line::from(all)
}

/// Milliseconds as `412 ms`, `3.4 s`, or `2m 05s`.
#[must_use]
pub fn duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        format!("{:.1} s", ms as f64 / 1000.0)
    } else {
        let secs = ms / 1000;
        format!("{}m {:02}s", secs / 60, secs % 60)
    }
}

/// A token count as `812`, `23.4k`, or `1.20M`.
#[must_use]
pub fn tokens(count: u64) -> String {
    if count < 1000 {
        count.to_string()
    } else if count < 1_000_000 {
        format!("{:.1}k", count as f64 / 1000.0)
    } else {
        format!("{:.2}M", count as f64 / 1e6)
    }
}

/// Dollars with enough places to show a small call.
#[must_use]
pub fn dollars(usd: f64) -> String {
    if usd == 0.0 {
        "$0".into()
    } else if usd < 0.0001 {
        "under $0.0001".into()
    } else if usd < 0.01 {
        format!("${usd:.4}")
    } else {
        format!("${usd:.2}")
    }
}

/// A ten-cell bar for a probability.
#[must_use]
pub fn bar(p: f64) -> String {
    let filled = (p.clamp(0.0, 1.0) * 10.0).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled))
}

fn label_of(kind: &str) -> &'static str {
    match kind {
        "model" => "Model",
        "ranked" => "Ranked",
        "briefing" => "Briefing",
        "agent" => "Agent",
        _ => "Stage",
    }
}

/// The lines of one decision card.
#[must_use]
pub fn decision_lines(
    card: &Value,
    output: &Value,
    running: bool,
    width: u16,
    phase: u8,
) -> Vec<Line<'static>> {
    let kind = card["kind"].as_str().unwrap_or("stage");
    let failed = output.get("error").is_some();
    let glyph = if running {
        crate::tools::spinner(phase)
    } else if failed {
        "×"
    } else {
        "◇"
    };
    let accent = if running {
        t::COMMAND
    } else if kind == "model" {
        t::ACCENT_MODEL
    } else {
        t::ACCENT_DELEGATE
    };
    let label = label_of(kind);
    let mut facts = Vec::new();
    if let Some(count) = card["count"].as_u64() {
        facts.push(format!("{count} files"));
    }
    if kind == "model" {
        if let Some(door) = card["door"].as_str() {
            facts.push(door.to_owned());
        }
        if let Some(model) = card["model"].as_str() {
            facts.push(model.to_owned());
        }
    }
    if let Some(ms) = card["ms"].as_u64() {
        facts.push(duration(ms));
    }
    if let Some(cost) = card["cost_usd"].as_f64() {
        facts.push(dollars(cost));
    }
    if let Some(tokens_in) = card["tokens"].as_u64().filter(|n| *n > 0) {
        facts.push(format!("{} tokens", tokens(tokens_in)));
    }
    let title = card["title"].as_str().unwrap_or_default().to_owned();
    let facts = if facts.is_empty() {
        String::new()
    } else {
        format!(" · {}", facts.join(" · "))
    };
    let room = width.saturating_sub(6 + label.len() as u16);
    let mut lines = vec![Line::from(vec![
        styled(
            format!("{glyph} "),
            if failed { t::DIFF_DELETE_FG } else { accent },
        ),
        Span::styled(
            label.to_owned(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            truncate(&title, room),
            Style::default()
                .fg(t::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        styled(
            truncate(&facts, room.saturating_sub(title.chars().count() as u16)),
            t::GRAY,
        ),
    ])];
    let inner = width.saturating_sub(RAIL.len() as u16);
    let text_row = |text: String, color: Color| rail(vec![styled(truncate(&text, inner), color)]);
    if let Some(detail) = card["detail"].as_str() {
        lines.push(text_row(detail.to_owned(), t::GRAY_BRIGHT));
    }
    match kind {
        "model" => model_lines(card, &mut lines, inner),
        "ranked" => {
            for row in card["rows"].as_array().into_iter().flatten() {
                let judged = row["judged"]
                    .as_f64()
                    .map(|p| format!("  answer {p:.2}"))
                    .unwrap_or_default();
                let path = row["path"].as_str().unwrap_or_default();
                lines.push(rail(vec![
                    styled(
                        format!("{:>2} ", row["rank"].as_u64().unwrap_or(0)),
                        t::GRAY,
                    ),
                    styled(
                        format!("{:.2} ", row["p"].as_f64().unwrap_or(0.0)),
                        t::ACCENT_SUCCESS,
                    ),
                    styled(truncate(path, inner.saturating_sub(18)), t::TEXT_SECONDARY),
                    styled(judged, t::ACCENT_MODEL),
                ]));
            }
        }
        "briefing" => {
            let field = |key: &str| card[key].as_str().map(str::to_owned);
            if let Some(chars) = card["chars"].as_u64() {
                lines.push(rail(vec![
                    styled("size     ", t::GRAY),
                    styled(
                        format!(
                            "{chars} characters · from {}",
                            field("finder").unwrap_or_default()
                        ),
                        t::TEXT_SECONDARY,
                    ),
                ]));
            }
            let list = |key: &str| -> Vec<String> {
                card[key]
                    .as_array()
                    .map(|v| {
                        v.iter()
                            .filter_map(|s| s.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            for path in list("files") {
                lines.push(rail(vec![
                    styled("file     ", t::GRAY),
                    styled(truncate(&path, inner.saturating_sub(9)), t::PATH),
                ]));
            }
            let checks = list("checks");
            if !checks.is_empty() {
                lines.push(rail(vec![
                    styled("checks   ", t::GRAY),
                    styled(
                        truncate(&checks.join(", "), inner.saturating_sub(9)),
                        t::TEXT_SECONDARY,
                    ),
                ]));
            }
            for step in list("plan") {
                lines.push(rail(vec![
                    styled("plan     ", t::GRAY),
                    styled(truncate(&step, inner.saturating_sub(9)), t::TEXT_SECONDARY),
                ]));
            }
            for change in list("history") {
                lines.push(rail(vec![
                    styled("past     ", t::GRAY),
                    styled(truncate(&change, inner.saturating_sub(9)), t::GRAY_BRIGHT),
                ]));
            }
            if let Some(path) = field("path") {
                lines.push(rail(vec![
                    styled("saved    ", t::GRAY),
                    styled(truncate(&path, inner.saturating_sub(9)), t::GRAY_BRIGHT),
                ]));
            }
        }
        _ => {
            let rows: Vec<&Value> = card["rows"].as_array().into_iter().flatten().collect();
            let label_width = rows
                .iter()
                .map(|row| row["label"].as_str().unwrap_or_default().chars().count())
                .max()
                .unwrap_or(0)
                .min(usize::from(inner / 2));
            for row in rows {
                let label = row["label"].as_str().unwrap_or_default();
                let label = truncate(label, label_width as u16);
                let pad = label_width.saturating_sub(label.chars().count());
                let is_path = label.contains('/') || label.contains('.');
                lines.push(rail(vec![
                    styled(
                        format!("{label}{}  ", " ".repeat(pad)),
                        if is_path { t::PATH } else { t::GRAY },
                    ),
                    styled(
                        truncate(
                            row["text"].as_str().unwrap_or_default(),
                            inner.saturating_sub(label_width as u16 + 2),
                        ),
                        t::TEXT_SECONDARY,
                    ),
                ]));
            }
            if let Some(more) = card["more"].as_u64().filter(|n| *n > 0) {
                lines.push(text_row(format!("… {more} more"), t::GRAY_DIM));
            }
        }
    }
    if running && kind != "model" {
        lines.push(text_row("Working".into(), accent));
    }
    if let Some(error) = output["error"].as_str() {
        lines.push(text_row(error.to_owned(), t::DIFF_DELETE_FG));
    }
    lines
}

fn model_lines(card: &Value, lines: &mut Vec<Line<'static>>, inner: u16) {
    if let Some(question) = card["question"].as_str() {
        lines.push(rail(vec![
            styled("? ", t::ACCENT_MODEL),
            styled(truncate(question, inner.saturating_sub(2)), t::TEXT_PRIMARY),
        ]));
    }
    if let Some(skipped) = card["skipped"].as_str() {
        lines.push(rail(vec![styled(
            truncate(&format!("Not asked: {skipped}"), inner),
            t::GRAY_BRIGHT,
        )]));
        return;
    }
    // A Choice: one row per option.
    if let Some(options) = card["options"].as_array() {
        let name_width = options
            .iter()
            .map(|o| o["name"].as_str().unwrap_or_default().chars().count())
            .max()
            .unwrap_or(0);
        for option in options {
            let name = option["name"].as_str().unwrap_or_default();
            let chosen = option["chosen"].as_bool() == Some(true);
            let mut spans = vec![styled(
                format!("  {name:<name_width$}  "),
                if chosen {
                    t::TEXT_PRIMARY
                } else {
                    t::GRAY_BRIGHT
                },
            )];
            match option["p"].as_f64() {
                Some(p) => {
                    spans.push(styled(
                        bar(p),
                        if chosen { t::ACCENT_MODEL } else { t::GRAY_DIM },
                    ));
                    spans.push(styled(format!(" {p:.2}"), t::GRAY_BRIGHT));
                    if chosen {
                        spans.push(styled("  ← answer", t::ACCENT_MODEL));
                    }
                }
                None => spans.push(styled("waiting", t::GRAY_DIM)),
            }
            lines.push(rail(spans));
        }
    }
    // Nouls: one row per file, with its probability of yes.
    if let Some(files) = card["files"].as_array() {
        for file in files {
            let name = file["name"].as_str().unwrap_or_default();
            let mut spans = vec![styled("  ", t::GRAY)];
            match (file["p"].as_f64(), file["error"].as_str()) {
                (Some(p), _) => {
                    let yes = file["chosen"].as_str() == Some("yes");
                    spans.push(styled(
                        bar(p),
                        if yes { t::ACCENT_MODEL } else { t::GRAY_DIM },
                    ));
                    spans.push(styled(format!(" {p:.2} "), t::GRAY_BRIGHT));
                    spans.push(styled(
                        if yes { "yes " } else { "no  " },
                        if yes {
                            t::ACCENT_SUCCESS
                        } else {
                            t::DIFF_DELETE_FG
                        },
                    ));
                }
                (None, Some(_)) => spans.push(styled("failed          ", t::DIFF_DELETE_FG)),
                (None, None) => spans.push(styled("…               ", t::GRAY_DIM)),
            }
            let ms = file["ms"]
                .as_u64()
                .map(|ms| format!("  {}", duration(ms)))
                .unwrap_or_default();
            spans.push(styled(
                truncate(name, inner.saturating_sub(22 + ms.len() as u16)),
                t::TEXT_SECONDARY,
            ));
            spans.push(styled(ms, t::GRAY_DIM));
            lines.push(rail(spans));
        }
    }
    if let Some(chosen) = card["chosen"].as_str() {
        lines.push(rail(vec![
            styled("→ ", t::ACCENT_MODEL),
            styled(chosen.to_owned(), t::TEXT_PRIMARY),
        ]));
    }
    if let Some(used) = card["used_for"].as_str() {
        lines.push(rail(vec![styled(truncate(used, inner), t::GRAY)]));
    }
}

fn names(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The summary's cost: the total only when every component is known,
/// otherwise the known subtotal and what is unknown. A missing amount is
/// never shown as $0 (#11230).
#[must_use]
pub fn cost_text(summary: &Value) -> String {
    let part = |usd: &Value| usd.as_f64().map_or_else(|| "unknown".to_owned(), dollars);
    let (agent, decisions) = if summary["cost"].is_object() {
        let parts = &summary["cost"]["components"];
        (
            parts["agent"]["usd"].clone(),
            parts["decisions"]["usd"].clone(),
        )
    } else {
        (
            summary["agent_usd"].clone(),
            summary["decision_usd"].clone(),
        )
    };
    let known: f64 = [&agent, &decisions].iter().filter_map(|v| v.as_f64()).sum();
    let head = if agent.is_number() && decisions.is_number() {
        dollars(known)
    } else {
        format!("unknown (known part {})", dollars(known))
    };
    format!(
        "{head} · the agent {} (as Claude Code reports it), the decisions {}",
        part(&agent),
        part(&decisions)
    )
}

/// The lines of the summary card.
#[must_use]
pub fn summary_lines(summary: &Value, output: &Value, width: u16) -> Vec<Line<'static>> {
    let accent = t::ACCENT_SUCCESS;
    let inner = width.saturating_sub(RAIL.len() as u16);
    let status = summary["outcome"]["status"].as_str();
    let failed = summary["error"].is_string() || status.is_some_and(|status| status != "passed");
    let mut lines = vec![Line::from(vec![
        styled(" ■ ", if failed { t::DIFF_DELETE_FG } else { accent }),
        Span::styled(
            "Summary",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            truncate(
                &format!(
                    "#{} {}",
                    summary["issue"].as_u64().unwrap_or(0),
                    summary["title"].as_str().unwrap_or_default()
                ),
                width.saturating_sub(12),
            ),
            Style::default()
                .fg(t::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
    ])];
    let row = |label: &str, text: String, color: Color| {
        rail(vec![
            styled(format!("{label:<10}"), t::GRAY),
            styled(truncate(&text, inner.saturating_sub(10)), color),
        ])
    };
    if let Some(status) = status {
        let delivers = summary["outcome"]["delivers"].as_bool() == Some(true);
        lines.push(row(
            "outcome",
            match summary["outcome"]["reason"].as_str() {
                Some(why) => format!("{status} · not delivered: {why}"),
                None => format!("{status} · every required check passed"),
            },
            if delivers {
                t::ACCENT_SUCCESS
            } else {
                t::DIFF_DELETE_FG
            },
        ));
    }
    let n = |key: &str| summary[key].as_u64().unwrap_or(0);
    lines.push(row(
        "time",
        format!(
            "{} in all · the agent {} over {} turns",
            duration(n("wall_ms")),
            duration(n("agent_ms")),
            n("turns")
        ),
        t::TEXT_SECONDARY,
    ));
    lines.push(row(
        "tokens",
        if summary["input_tokens"].is_null() && summary["output_tokens"].is_null() {
            format!(
                "unknown ({})",
                summary["usage_unknown_reason"]
                    .as_str()
                    .unwrap_or("the session reported no usage")
            )
        } else {
            format!(
                "{} in ({} read from cache, {} written to it) · {} out",
                tokens(n("input_tokens") + n("cache_read_tokens") + n("cache_write_tokens")),
                tokens(n("cache_read_tokens")),
                tokens(n("cache_write_tokens")),
                tokens(n("output_tokens")),
            )
        },
        t::TEXT_SECONDARY,
    ));
    lines.push(row("cost", cost_text(summary), t::TEXT_SECONDARY));
    let verdicts = |key: &str| -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        for check in summary[key].as_array().into_iter().flatten() {
            let ok = check["ok"].as_bool() == Some(true);
            spans.push(styled(
                format!("{} ", check["id"].as_str().unwrap_or_default()),
                t::TEXT_SECONDARY,
            ));
            spans.push(styled(
                if ok { "passed  " } else { "failed  " },
                if ok {
                    t::ACCENT_SUCCESS
                } else {
                    t::DIFF_DELETE_FG
                },
            ));
        }
        if spans.is_empty() {
            spans.push(styled("none run", t::GRAY_BRIGHT));
        }
        spans
    };
    let mut checks = vec![styled(RAIL, t::GRAY_DIM), styled("checks    ", t::GRAY)];
    checks.extend(verdicts("checks"));
    lines.push(Line::from(checks));
    let mut agent_checks = vec![styled(RAIL, t::GRAY_DIM), styled("agent ran ", t::GRAY)];
    agent_checks.extend(verdicts("agent_checks"));
    lines.push(Line::from(agent_checks));
    let optional: Vec<String> = summary["optional_checks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|check| {
            let id = check["id"].as_str().unwrap_or_default();
            match check["ok"].as_bool() {
                Some(true) => format!("{id} passed"),
                Some(false) => format!("{id} failed"),
                None => format!("{id} not run"),
            }
        })
        .collect();
    if !optional.is_empty() {
        lines.push(row(
            "optional",
            format!(
                "{} (reported, never deciding the outcome)",
                optional.join(", ")
            ),
            t::GRAY_BRIGHT,
        ));
    }
    let outside = names(&summary["opened_outside_briefing"]);
    lines.push(row(
        "outside",
        if outside.is_empty() {
            "The agent opened no file the briefing did not list".into()
        } else {
            format!(
                "{} not in the briefing: {}",
                outside.len(),
                outside.join(", ")
            )
        },
        if outside.is_empty() {
            t::ACCENT_SUCCESS
        } else {
            t::COMMAND
        },
    ));
    let changed = names(&summary["changed"]);
    lines.push(row(
        "diff",
        format!(
            "{} {} +{} -{}{}",
            changed.len(),
            if changed.len() == 1 { "file" } else { "files" },
            n("added"),
            n("removed"),
            if changed.is_empty() {
                String::new()
            } else {
                format!(": {}", changed.join(", "))
            }
        ),
        t::TEXT_SECONDARY,
    ));
    if let Some(fix) = summary.get("fix").filter(|f| f.is_object()) {
        let files = names(&fix["files"]);
        let both = names(&fix["both"]);
        let missed = names(&fix["missed"]);
        let briefed = names(&fix["briefed"]);
        lines.push(row(
            "real fix",
            format!(
                "{} changed {} {}; the agent changed {} of them; the briefing listed {}",
                fix["commit"].as_str().unwrap_or_default(),
                files.len(),
                if files.len() == 1 { "file" } else { "files" },
                both.len(),
                briefed.len()
            ),
            t::TEXT_SECONDARY,
        ));
        if !missed.is_empty() {
            lines.push(row(
                "not done",
                format!("The real fix also changed {}", missed.join(", ")),
                t::COMMAND,
            ));
        }
    }
    if let Some(error) = summary["error"].as_str() {
        lines.push(row("stopped", error.to_owned(), t::DIFF_DELETE_FG));
    }
    if let Some(reply) = summary["reply"].as_str().filter(|r| !r.trim().is_empty()) {
        for (i, line) in reply
            .trim()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(4)
            .enumerate()
        {
            lines.push(row(
                if i == 0 { "reply" } else { "" },
                line.to_owned(),
                t::GRAY_BRIGHT,
            ));
        }
    }
    lines.push(row(
        "kept in",
        summary["folder"].as_str().unwrap_or_default().to_owned(),
        t::GRAY_BRIGHT,
    ));
    // The diff, file by file, with Coder's diff renderer.
    let mut shown = 0;
    for (path, hunks) in split_diff(output["diff"].as_str().unwrap_or_default()) {
        if shown >= 160 {
            lines.push(rail(vec![styled(
                "… the rest of the diff is in the run's folder",
                t::GRAY_DIM,
            )]));
            break;
        }
        lines.push(Line::from(vec![
            styled("   ", t::GRAY_DIM),
            styled(truncate(&path, width.saturating_sub(3)), t::PATH),
        ]));
        let drawn = t::noir_lines(diff::lines(
            &hunks,
            &path,
            3,
            usize::from(width),
            Palette::Night,
            ColorLevel::TrueColor,
        ));
        let take = drawn.len().min(80);
        shown += take;
        lines.extend(drawn.into_iter().take(take));
    }
    lines
}

/// A `git diff` split into `(path, hunks)` per file; a binary or empty
/// file's entry has no hunks and is left out.
#[must_use]
pub fn split_diff(diff: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for block in diff
        .split("\ndiff --git ")
        .map(|b| b.trim_start_matches("diff --git "))
    {
        let Some(path) = block
            .lines()
            .next()
            .and_then(|head| head.split_once(" b/"))
            .map(|(_, path)| path.to_owned())
        else {
            continue;
        };
        if let Some(start) = block.find("\n@@") {
            let mut hunks = block[start + 1..].to_owned();
            if !hunks.ends_with('\n') {
                hunks.push('\n');
            }
            files.push((path, hunks));
        }
    }
    files
}
