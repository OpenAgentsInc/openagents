//! Long sessions (#11179): compacting a chat near the model's context
//! limit, pausing on a usage limit and resuming on its own, and the chat's
//! dollars on the status line.
//!
//! **Compaction.** When the conversation the model reads grows past
//! [`COMPACT_PERCENT`] of the context window, or when the person types
//! `/compact`, the older part of the chat becomes one summary entry. The
//! newest [`KEEP_TURNS`] requests stay word for word. The transcript keeps
//! every earlier message on screen; only what the model reads changes:
//! [`crate::live::Chat::messages`] starts at the newest summary. With an
//! OpenRouter key the summary is written by the chat's model; without one,
//! or when that request fails, a plain summary is built here from the
//! requests, replies and commands themselves.
//!
//! **Usage limits.** A rate limit (HTTP 429) inside a reply already waits
//! and retries in the provider loop; the loop now shows that wait as a
//! "Paused until" row. A turn that ends on a usage limit pauses the chat
//! until the reset time, holds new prompts in the queue, and then starts
//! the next turn on its own. Background agents keep running throughout.
//!
//! **Dollars.** Each reply's provider-reported cost is added to its chat;
//! a finished delegation's cost is kept on its agent. The status line shows
//! the session's total.

use std::time::Duration;

use openrouter::Message;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use serde_json::{Value, json};

use crate::bundled_runtime::RuntimeEvent;
use crate::live::{self, Entry};
use crate::{App, Mode, theme as t};

/// The transcript entry that holds a summary of the earlier chat.
pub const COMPACT_TOOL: &str = "compact";
/// The transcript entry shown while a reply waits out a usage limit.
pub const LIMIT_TOOL: &str = "usage_limit";
/// Requests kept word for word after a compaction.
pub const KEEP_TURNS: usize = 3;
/// The context window assumed when the model does not say.
pub const DEFAULT_CONTEXT_TOKENS: u64 = 200_000;
/// Compaction starts at this share of the context window.
pub const COMPACT_PERCENT: u64 = 80;
/// How long to pause when a usage limit names no reset time.
const DEFAULT_PAUSE_SECS: u64 = 60;
/// Waits shorter than this inside a reply are not shown as a pause.
const SHOWN_PAUSE: Duration = Duration::from_secs(5);
/// The summary request's transcript allowance, in bytes.
const SUMMARY_INPUT_BYTES: usize = 400 * 1024;
/// One message's allowance inside the summary request.
const SUMMARY_MESSAGE_BYTES: usize = 8 * 1024;
/// The plain summary's allowance.
const LOCAL_SUMMARY_BYTES: usize = 12 * 1024;

/// What the next turn says after a usage limit resets.
pub const RESUME_TEXT: &str = "The usage limit has reset. Continue the task from where you stopped; do not repeat finished steps.";

/// The chat's long-session state.
#[derive(Debug, Default)]
pub(crate) struct State {
    /// A turn ended on a usage limit: no new turn starts before this.
    pub paused_until_ms: Option<u64>,
    /// A summary request in flight.
    compacting: Option<Job>,
    /// The entry count at the last size check, so the check runs once per
    /// change rather than every frame.
    checked: usize,
    /// The model's context window, when known; [`DEFAULT_CONTEXT_TOKENS`]
    /// otherwise.
    pub context_tokens: Option<u64>,
}

#[derive(Debug)]
struct Job {
    request_id: u64,
    start: usize,
    cut: usize,
}

impl State {
    pub(crate) fn paused(&self) -> bool {
        self.paused_until_ms
            .is_some_and(|until| atif::now_ms() < until)
    }

    fn limit(&self) -> u64 {
        self.context_tokens
            .filter(|tokens| *tokens > 0)
            .unwrap_or_else(context_tokens_from_env)
    }
}

fn context_tokens_from_env() -> u64 {
    std::env::var("OPENAGENTS_CODER_CONTEXT_TOKENS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|tokens| *tokens >= 1_000)
        .unwrap_or(DEFAULT_CONTEXT_TOKENS)
}

/// Whether an entry is a finished summary.
pub(crate) fn is_summary(entry: &Entry) -> bool {
    matches!(entry, Entry::Tool { name, output, running: false, .. }
        if name == COMPACT_TOOL && output["summary"].is_string())
}

/// Where the model's view of the chat starts: the newest summary, or the
/// first entry.
pub(crate) fn context_start(entries: &[Entry]) -> usize {
    entries.iter().rposition(is_summary).unwrap_or(0)
}

/// What the model reads for a summary entry.
pub(crate) fn summary_message(output: &Value) -> Option<Message> {
    let summary = output["summary"].as_str()?;
    Some(Message::user(format!(
        "Summary of the earlier part of this conversation, compacted to fit the context window:\n\n{summary}"
    )))
}

/// Entries the model never reads: the pause rows.
pub(crate) fn hidden_from_model(name: &str) -> bool {
    name == LIMIT_TOOL
}

/// A rough token count for what the model reads: four bytes a token, plus
/// each message's framing.
pub(crate) fn estimate_tokens(messages: &[Message]) -> u64 {
    messages
        .iter()
        .map(|message| (message.content.len() as u64 + 16) / 4)
        .sum()
}

/// The range a compaction folds into one summary: from the newest summary
/// (or the start) up to the newest [`KEEP_TURNS`] requests, or up to the
/// latest request when there are not that many. `None` when nothing older
/// than what is kept remains.
pub(crate) fn plan(entries: &[Entry]) -> Option<(usize, usize)> {
    let start = context_start(entries);
    let requests: Vec<usize> = entries
        .iter()
        .enumerate()
        .skip(start)
        .filter(|(_, entry)| matches!(entry, Entry::User(_)))
        .map(|(index, _)| index)
        .collect();
    let keep = if requests.len() > KEEP_TURNS {
        KEEP_TURNS
    } else {
        1
    };
    let cut = *requests.get(requests.len().checked_sub(keep)?)?;
    let folded = entries[start..cut]
        .iter()
        .filter(|entry| !is_summary(entry))
        .count();
    (folded > 0).then_some((start, cut))
}

/// The model's view of `entries`, as [`live::Chat::messages`] builds it.
fn messages_of(entries: &[Entry]) -> Vec<Message> {
    live::Chat {
        entries: entries.to_vec(),
        ..live::Chat::default()
    }
    .messages()
}

/// The request that asks the chat's model for a summary.
pub(crate) fn summary_request(older: &[Message]) -> Vec<Message> {
    let mut rows = Vec::new();
    let mut bytes = 0;
    for message in older.iter().rev() {
        let row = format!(
            "{}: {}",
            message.role,
            clip(&message.content, SUMMARY_MESSAGE_BYTES)
        );
        if bytes + row.len() > SUMMARY_INPUT_BYTES {
            rows.push("(earlier messages left out to fit)".to_owned());
            break;
        }
        bytes += row.len() + 2;
        rows.push(row);
    }
    rows.reverse();
    vec![
        Message::system(
            "You compact a coding conversation so it can continue with less text. Write a summary the same assistant can continue from without the original messages. Keep: what the user asked for and every constraint they set; decisions made and why; files, branches, commands and results that still matter; what is finished; what is left; open questions; any agents or commands still running. Use short Markdown sections and lists. State only what the conversation shows.",
        ),
        Message::user(format!(
            "The conversation to summarize:\n\n{}\n\nWrite the summary now.",
            rows.join("\n\n")
        )),
    ]
}

/// A plain summary of `older`, built without a model: the requests, the
/// latest replies, and the commands and tools used.
pub(crate) fn local_summary(entries: &[Entry]) -> String {
    let mut requests = Vec::new();
    let mut replies = Vec::new();
    let mut commands = Vec::new();
    let mut tools: Vec<(String, usize)> = Vec::new();
    let mut earlier = None;
    for entry in entries {
        match entry {
            Entry::User(text) => requests.push(one_line(text, 300)),
            Entry::Assistant { text, .. } => replies.push(one_line(text, 400)),
            Entry::Tool { name, output, .. } if name == COMPACT_TOOL => {
                earlier = output["summary"].as_str().map(|text| clip(text, 4 * 1024));
            }
            Entry::Tool { name, input, .. } => {
                if name == "Run"
                    && let Some(command) = input["command"].as_str()
                {
                    commands.push(one_line(command, 160));
                }
                if !hidden_from_model(name) {
                    match tools.iter_mut().find(|(seen, _)| seen == name) {
                        Some((_, count)) => *count += 1,
                        None => tools.push((name.clone(), 1)),
                    }
                }
            }
            Entry::Delegation { name, task, .. } => {
                requests.push(format!("(to {name}) {}", one_line(task, 200)));
            }
        }
    }
    let mut text = String::new();
    if let Some(earlier) = earlier {
        text.push_str("Earlier summary:\n");
        text.push_str(&earlier);
        text.push_str("\n\n");
    }
    if !requests.is_empty() {
        text.push_str("Requests, oldest first:\n");
        for request in &requests {
            text.push_str(&format!("- {request}\n"));
        }
    }
    if !replies.is_empty() {
        text.push_str("\nLatest replies:\n");
        for reply in replies.iter().rev().take(3).rev() {
            text.push_str(&format!("- {reply}\n"));
        }
    }
    if !tools.is_empty() {
        text.push_str("\nTools used: ");
        text.push_str(
            &tools
                .iter()
                .map(|(name, count)| format!("{name} ×{count}"))
                .collect::<Vec<_>>()
                .join(", "),
        );
        text.push('\n');
    }
    if !commands.is_empty() {
        text.push_str("\nLatest commands:\n");
        for command in commands.iter().rev().take(10).rev() {
            text.push_str(&format!("- {command}\n"));
        }
    }
    clip(text.trim(), LOCAL_SUMMARY_BYTES)
}

fn one_line(text: &str, bytes: usize) -> String {
    clip(
        &text.split_whitespace().collect::<Vec<_>>().join(" "),
        bytes,
    )
}

/// `text` cut to at most `bytes` on a character boundary, marked when cut.
pub(crate) fn clip(text: &str, bytes: usize) -> String {
    if text.len() <= bytes {
        return text.to_owned();
    }
    let mut end = bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// The error a rate-limited reply ends with, naming the reset time when
/// the provider sent one.
pub(crate) fn rate_limited(retry_after: Option<u64>) -> String {
    match retry_after {
        Some(seconds) => {
            format!("OpenRouter is rate limited (HTTP 429). It resets in {seconds} seconds.")
        }
        None => "OpenRouter is rate limited (HTTP 429). Try again later.".into(),
    }
}

/// How long to pause after a turn ended with `error`: `None` unless it is
/// a usage limit. Reads only the errors this crate writes.
pub(crate) fn pause_seconds(error: &str) -> Option<u64> {
    if !error.contains("(HTTP 429)") {
        return None;
    }
    let seconds = error
        .split_once("resets in ")
        .and_then(|(_, rest)| {
            rest.split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|digits| digits.parse::<u64>().ok())
        })
        .unwrap_or(DEFAULT_PAUSE_SECS);
    Some(seconds.clamp(1, 86_400))
}

/// The row a reply shows while it waits out a usage limit, or `None` for
/// a short wait or any other failure.
pub(crate) fn pause_event(error: &openrouter::Error, wait: Duration) -> Option<RuntimeEvent> {
    let openrouter::Error::Api { status: 429, .. } = error else {
        return None;
    };
    if wait < SHOWN_PAUSE {
        return None;
    }
    let until = atif::now_ms().saturating_add(u64::try_from(wait.as_millis()).unwrap_or(u64::MAX));
    Some(RuntimeEvent::Tool {
        name: LIMIT_TOOL.into(),
        input: json!({"limit": "rate"}),
        output: json!({"paused_until_ms": until}),
        running: true,
    })
}

/// The same row once the reply goes on.
pub(crate) fn resume_event() -> RuntimeEvent {
    RuntimeEvent::Tool {
        name: LIMIT_TOOL.into(),
        input: Value::Null,
        output: json!({"resumed_at_ms": atif::now_ms()}),
        running: false,
    }
}

/// The local time of day of `ms`, as `HH:MM`.
pub(crate) fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    #[cfg(unix)]
    let offset = background::engine::local_offset(seconds);
    #[cfg(not(unix))]
    let offset = 0_i64;
    let local = i128::from(seconds) + i128::from(offset);
    let minutes = local.rem_euclid(86_400) / 60;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// The cost a finished delegation's result reports.
pub(crate) fn output_cost(output: &Value) -> Option<f64> {
    crate::fleet::usage(output)
        .1
        .filter(|cost| cost.is_finite())
}

/// When the chat may start its next turn, while it is paused.
pub(crate) fn paused_until(app: &App) -> Option<u64> {
    let now = atif::now_ms();
    app.long_session
        .paused_until_ms
        .filter(|until| *until > now)
        .or_else(|| {
            if !app.live.busy {
                return None;
            }
            app.live.entries.iter().rev().find_map(|entry| match entry {
                Entry::Tool {
                    name,
                    output,
                    running: true,
                    ..
                } if name == LIMIT_TOOL => output["paused_until_ms"].as_u64(),
                _ => None,
            })
        })
}

/// The session's dollars: the chat's own replies plus every agent's.
pub(crate) fn session_cost(app: &App) -> f64 {
    let agents: f64 = app
        .delegations
        .iter()
        .map(|agent| {
            if agent.background {
                app.fleet
                    .get(&agent.id)
                    .and_then(|row| row.cost_usd)
                    .unwrap_or(agent.chat.cost_usd)
            } else {
                agent.chat.cost_usd
            }
        })
        .filter(|cost| cost.is_finite())
        .sum();
    app.live.cost_usd + agents
}

/// The status line's long-session words: a pause and the dollars spent.
#[cfg(test)]
pub(crate) fn status(app: &App) -> Option<String> {
    if app.mode != Mode::Live {
        return None;
    }
    let mut parts = Vec::new();
    if let Some(until) = paused_until(app) {
        parts.push(format!("Paused until {}", clock(until)));
    }
    let cost = session_cost(app);
    if cost > 0.0 {
        parts.push(agent_fleet::dollars(cost));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// How the transcript draws a summary or a pause row; `None` for any other
/// entry.
pub(crate) fn entry_lines(
    name: &str,
    input: &Value,
    output: &Value,
    running: bool,
    width: u16,
    phase: u8,
) -> Option<Vec<Line<'static>>> {
    let styled = |text: String, color: Color| -> Span<'static> {
        Span::styled(text, Style::default().fg(color))
    };
    let bold = |text: &str, color: Color| -> Span<'static> {
        Span::styled(
            text.to_owned(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )
    };
    match name {
        COMPACT_TOOL => {
            let count = input["messages"].as_u64().unwrap_or(0);
            let mut lines = vec![Line::from(vec![
                styled("● ".into(), t::ACCENT_SKILL),
                bold("Compacted", t::ACCENT_SKILL),
                styled(
                    crate::ui::truncate(
                        &format!(" {count} earlier messages into a summary"),
                        width.saturating_sub(13),
                    ),
                    t::TEXT_PRIMARY,
                ),
            ])];
            let summary = output["summary"].as_str().unwrap_or_default();
            let rows: Vec<&str> = summary
                .lines()
                .filter(|row| !row.trim().is_empty())
                .collect();
            for row in rows.iter().take(6) {
                lines.push(Line::from(vec![
                    styled("  │  ".into(), t::GRAY_DIM),
                    styled(
                        crate::ui::truncate(row, width.saturating_sub(5)),
                        t::GRAY_BRIGHT,
                    ),
                ]));
            }
            if rows.len() > 6 {
                lines.push(Line::from(vec![
                    styled("  ⎿  ".into(), t::GRAY_DIM),
                    styled(format!("{} more lines", rows.len() - 6), t::GRAY),
                ]));
            }
            Some(lines)
        }
        LIMIT_TOOL => {
            let line = if running {
                let until = output["paused_until_ms"]
                    .as_u64()
                    .map(clock)
                    .unwrap_or_else(|| "the reset".into());
                Line::from(vec![
                    styled(format!("{} ", crate::tools::spinner(phase)), t::COMMAND),
                    bold("Paused", t::COMMAND),
                    styled(
                        crate::ui::truncate(
                            &format!(" until {until} on a usage limit; it goes on by itself"),
                            width.saturating_sub(10),
                        ),
                        t::GRAY_BRIGHT,
                    ),
                ])
            } else if output.get("error").is_some() {
                Line::from(vec![
                    styled("× ".into(), t::DIFF_DELETE_FG),
                    bold("Paused", t::DIFF_DELETE_FG),
                    styled(" on a usage limit, then stopped".into(), t::GRAY_BRIGHT),
                ])
            } else {
                Line::from(vec![
                    styled("● ".into(), t::ACCENT_SUCCESS),
                    bold("Resumed", t::ACCENT_SUCCESS),
                    styled(" after a usage limit".into(), t::GRAY_BRIGHT),
                ])
            };
            Some(vec![line])
        }
        _ => None,
    }
}

impl App {
    /// `/compact`: fold the older part of the chat into a summary now.
    pub(crate) fn compact_command(&mut self) {
        if self.mode != Mode::Live {
            self.notice = Some("Compacting works in live mode only.".into());
            return;
        }
        self.start_compaction(true);
    }

    /// Starts a compaction. A manual one explains why it cannot start.
    fn start_compaction(&mut self, manual: bool) -> bool {
        if self.live.busy
            || self.request.is_some()
            || self.checking_key
            || self.checking_jev
            || self.brainstorm_job.is_some()
        {
            if manual {
                self.notice = Some("Wait for the reply to finish, then run /compact again.".into());
            }
            return false;
        }
        let Some((start, cut)) = plan(&self.live.entries) else {
            if manual {
                self.notice = Some("There is nothing older to compact yet.".into());
            }
            return false;
        };
        let key = self
            .plugins
            .key_for_request()
            .filter(|_| self.plugins.enabled);
        let Some(key) = key else {
            let summary = local_summary(&self.live.entries[start..cut]);
            self.apply_summary(start, cut, summary, false);
            return true;
        };
        let older = messages_of(&self.live.entries[start..cut]);
        self.cancel_request();
        self.live.busy = true;
        self.live.partial.clear();
        self.live.partial_model = None;
        self.live.reply_started_at = Some(std::time::Instant::now());
        self.live.notice = Some("Compacting the conversation to fit the context window…".into());
        self.long_session.compacting = Some(Job {
            request_id: self.request_id,
            start,
            cut,
        });
        self.request = Some(live::Request {
            id: self.request_id,
            key,
            kind: live::Work::Summarize {
                model: self.plugins.model.clone(),
                messages: summary_request(&older),
            },
        });
        true
    }

    /// Takes the summary request's result; any other update passes on.
    pub(crate) fn finish_compaction(&mut self, update: live::Update) -> Option<live::Update> {
        if self
            .long_session
            .compacting
            .as_ref()
            .is_none_or(|job| job.request_id != update.id())
        {
            return Some(update);
        }
        let live::Update::Finished { result, .. } = update else {
            return None;
        };
        let job = self.long_session.compacting.take()?;
        self.live.busy = false;
        self.live.notice = None;
        if job.cut > self.live.entries.len() || job.start >= job.cut {
            return None;
        }
        let summary = match result {
            Ok(reply) => {
                self.live.tokens = self.live.tokens.saturating_add(reply.usage.total_tokens);
                if let Some(cost) = reply.usage.cost.filter(|cost| cost.is_finite()) {
                    self.live.cost_usd += cost;
                }
                Some(reply.text).filter(|text| !text.trim().is_empty())
            }
            Err(_) => None,
        };
        let by_model = summary.is_some();
        let summary =
            summary.unwrap_or_else(|| local_summary(&self.live.entries[job.start..job.cut]));
        self.apply_summary(job.start, job.cut, summary, by_model);
        None
    }

    fn apply_summary(&mut self, start: usize, cut: usize, summary: String, by_model: bool) {
        if cut > self.live.entries.len() || start >= cut {
            return;
        }
        let count = self.live.entries[start..cut]
            .iter()
            .filter(|entry| !is_summary(entry))
            .count();
        self.live.entries.insert(
            cut,
            Entry::Tool {
                name: COMPACT_TOOL.into(),
                input: json!({"messages": count}),
                output: json!({"summary": summary, "by_model": by_model}),
                running: false,
            },
        );
        self.live.notice = Some(format!(
            "Compacted {count} earlier messages into a summary. They stay above; the model now reads the summary instead."
        ));
        self.long_session.checked = self.live.entries.len();
        self.history.dirty = true;
        self.scroll_main_to_end();
    }

    /// After a turn fails: a usage limit pauses the chat until it resets.
    pub(crate) fn note_turn_error(&mut self) {
        let Some(seconds) = self.live.notice.as_deref().and_then(pause_seconds) else {
            return;
        };
        let until = atif::now_ms().saturating_add(seconds.saturating_mul(1000));
        self.long_session.paused_until_ms = Some(until);
        self.live.notice = Some(format!(
            "Paused until {} on a usage limit. Coder goes on by itself then; background agents keep running.",
            clock(until)
        ));
    }

    /// Runs every frame: ends a pause, repeats due loops, and compacts a
    /// chat that grew near the context limit.
    pub(crate) fn poll_long_session(&mut self) {
        if self
            .long_session
            .compacting
            .as_ref()
            .is_some_and(|job| job.request_id != self.request_id)
        {
            self.long_session.compacting = None;
        }
        if let Some(until) = self.long_session.paused_until_ms
            && atif::now_ms() >= until
        {
            self.long_session.paused_until_ms = None;
            if self.mode == Mode::Live {
                self.live.notice = None;
                self.queue_notice(RESUME_TEXT.into());
            }
        }
        self.poll_loops();
        if self.mode != Mode::Live
            || self.live.busy
            || self.request.is_some()
            || self.long_session.paused()
            || self.live.entries.len() == self.long_session.checked
        {
            return;
        }
        self.long_session.checked = self.live.entries.len();
        let mut used = estimate_tokens(&self.live.messages());
        if let Some(instructions) = &self.live.instructions {
            used += instructions.len() as u64 / 4;
        }
        if used.saturating_mul(100) >= self.long_session.limit().saturating_mul(COMPACT_PERCENT) {
            self.start_compaction(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(index: usize, size: usize) -> [Entry; 2] {
        [
            Entry::User(format!("request {index}")),
            Entry::Assistant {
                text: format!("reply {index} {}", "x".repeat(size)),
                model: Some("fixture/model".into()),
                elapsed_ms: None,
            },
        ]
    }

    fn long_chat(turns: usize, size: usize) -> Vec<Entry> {
        (0..turns).flat_map(|index| turn(index, size)).collect()
    }

    fn live_app() -> App {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app
    }

    fn keyed(app: &mut App) {
        app.plugins
            .bootstrap_credentials(crate::credentials::Imported {
                openrouter_key: Some(model_access::ApiKey::new("fixture-long-session-key")),
                jev_key: None,
                jev_endpoint: None,
                gateway_key: None,
                jev_model: None,
            });
        assert!(app.plugins.enabled && app.plugins.key_configured);
    }

    #[test]
    fn a_plan_keeps_the_newest_requests_and_folds_the_rest() {
        let entries = long_chat(6, 10);
        let (start, cut) = plan(&entries).unwrap();
        assert_eq!(start, 0);
        assert_eq!(cut, 6, "the last three requests start at entry six");
        assert!(plan(&long_chat(1, 10)).is_none());
        // Two requests: the latest is kept, the first folds.
        assert_eq!(plan(&long_chat(2, 10)), Some((0, 2)));
    }

    #[test]
    fn a_synthetic_long_chat_compacts_and_continues() {
        let mut app = live_app();
        app.long_session.context_tokens = Some(2_000);
        app.live.entries = long_chat(12, 1_000);
        let before = app.live.entries.len();
        app.poll_long_session();
        // No key: the plain summary applies at once.
        assert!(!app.live.busy);
        assert_eq!(app.live.entries.len(), before + 1);
        let start = context_start(&app.live.entries);
        assert!(is_summary(&app.live.entries[start]));
        let Entry::Tool { output, input, .. } = &app.live.entries[start] else {
            unreachable!()
        };
        assert_eq!(input["messages"], 18);
        let summary = output["summary"].as_str().unwrap();
        assert!(summary.contains("request 0"));
        assert!(summary.contains("request 8"));
        // The model reads the summary and the three newest turns only.
        let messages = app.live.messages();
        assert_eq!(messages.len(), 1 + 3 * 2);
        assert!(
            messages[0]
                .content
                .starts_with("Summary of the earlier part")
        );
        assert_eq!(messages[1].content, "request 9");
        assert!(estimate_tokens(&messages) < estimate_tokens(&messages_of(&long_chat(12, 1_000))));
        // Every earlier message stays in the transcript.
        assert!(matches!(&app.live.entries[0], Entry::User(text) if text == "request 0"));
        // The chat goes on: a new turn starts from the compacted view.
        app.submit("request 12", std::path::Path::new("."));
        let request = app.request.take().expect("the next turn starts");
        let live::Work::Microcoder { messages, .. } = request.kind else {
            panic!("a keyless chat uses the local loop")
        };
        assert!(
            messages[0]
                .content
                .starts_with("Summary of the earlier part")
        );
        assert_eq!(messages.last().unwrap().content, "request 12");
    }

    #[test]
    fn a_second_compaction_folds_the_first_summary_in() {
        let mut app = live_app();
        app.long_session.context_tokens = Some(2_000);
        app.live.entries = long_chat(12, 1_000);
        app.poll_long_session();
        app.live.entries.extend(long_chat(6, 1_000));
        app.poll_long_session();
        let summaries = app
            .live
            .entries
            .iter()
            .filter(|entry| is_summary(entry))
            .count();
        assert_eq!(summaries, 2);
        let messages = app.live.messages();
        assert!(messages[0].content.contains("Earlier summary:"));
        assert_eq!(messages.len(), 1 + 3 * 2);
    }

    #[test]
    fn compact_with_a_key_asks_the_model_and_falls_back_to_a_plain_summary() {
        let mut app = live_app();
        app.live.entries = long_chat(5, 10);
        keyed(&mut app);
        app.compact_command();
        assert!(app.live.busy);
        let request = app.request.take().unwrap();
        let live::Work::Summarize { messages, .. } = &request.kind else {
            panic!("a summary request")
        };
        assert_eq!(messages[0].role, "system");
        assert!(messages[1].content.contains("request 0"));
        assert!(!messages[1].content.contains("request 4"));
        // A queued prompt waits for the summary.
        app.apply_update(live::Update::Finished {
            id: request.id,
            result: Err("OpenRouter could not complete this request (HTTP 500).".into()),
        });
        assert!(!app.live.busy);
        assert!(is_summary(
            &app.live.entries[context_start(&app.live.entries)]
        ));
    }

    #[test]
    fn a_model_summary_replaces_the_older_turns() {
        let mut app = live_app();
        app.live.entries = long_chat(5, 10);
        keyed(&mut app);
        app.compact_command();
        let request = app.request.take().unwrap();
        let mut reply = openrouter::Streamed {
            text: "## Goal\nShip the parser.".into(),
            ..openrouter::Streamed::default()
        };
        reply.usage.cost = Some(0.25);
        app.apply_update(live::Update::Finished {
            id: request.id,
            result: Ok(reply),
        });
        let start = context_start(&app.live.entries);
        let Entry::Tool { output, .. } = &app.live.entries[start] else {
            unreachable!()
        };
        assert_eq!(output["summary"], "## Goal\nShip the parser.");
        assert_eq!(output["by_model"], true);
        assert!((app.live.cost_usd - 0.25).abs() < 1e-9);
        assert_eq!(status(&app).as_deref(), Some("$0.25"));
    }

    #[test]
    fn a_rate_limit_with_a_reset_time_pauses_and_resumes() {
        let mut app = live_app();
        app.submit("first", std::path::Path::new("."));
        let request = app.request.take().unwrap();
        app.apply_update(live::Update::Finished {
            id: request.id,
            result: Err(rate_limited(Some(120))),
        });
        assert!(app.long_session.paused());
        let notice = app.live.notice.clone().unwrap();
        assert!(notice.starts_with("Paused until "), "{notice}");
        assert!(status(&app).unwrap().starts_with("Paused until "));
        // A new prompt waits in the queue while paused.
        app.submit("second", std::path::Path::new("."));
        assert!(app.request.is_none());
        app.process_prompt_queue();
        assert!(app.request.is_none());
        // The reset passes: the chat goes on by itself.
        app.long_session.paused_until_ms = Some(atif::now_ms().saturating_sub(1));
        app.poll_long_session();
        app.process_prompt_queue();
        assert!(!app.long_session.paused());
        let request = app.request.take().expect("the chat resumes on its own");
        let live::Work::Microcoder { messages, .. } = request.kind else {
            panic!("a keyless chat uses the local loop")
        };
        let texts: Vec<_> = messages.iter().map(|m| m.content.as_str()).collect();
        assert!(texts.contains(&RESUME_TEXT));
        assert!(texts.contains(&"second"));
    }

    #[test]
    fn pause_seconds_reads_only_usage_limits() {
        assert_eq!(pause_seconds(&rate_limited(Some(90))), Some(90));
        assert_eq!(pause_seconds(&rate_limited(None)), Some(DEFAULT_PAUSE_SECS));
        assert_eq!(
            pause_seconds("OpenRouter rejected the API key (HTTP 401)."),
            None
        );
        let fallback = format!(
            "{} The alternate providers could not continue: none.",
            rate_limited(Some(30))
        );
        assert_eq!(pause_seconds(&fallback), Some(30));
    }

    #[test]
    fn a_pause_inside_a_reply_shows_until_it_goes_on() {
        let error = openrouter::Error::Api {
            kind: openrouter::ApiErrorKind::of(429),
            status: 429,
            message: String::new(),
            retry_after: Some(300),
            body: String::new(),
        };
        assert!(pause_event(&error, Duration::from_secs(1)).is_none());
        let Some(RuntimeEvent::Tool {
            name,
            input,
            output,
            running,
        }) = pause_event(&error, Duration::from_secs(300))
        else {
            panic!("a long wait shows a pause")
        };
        let mut app = live_app();
        app.submit("work", std::path::Path::new("."));
        let id = app.request.take().unwrap().id;
        app.apply_update(live::Update::Tool {
            id,
            name,
            input,
            output,
            running,
        });
        assert!(status(&app).unwrap().starts_with("Paused until "));
        // The model never reads the pause row.
        assert_eq!(app.live.messages().len(), 1);
        let RuntimeEvent::Tool {
            name,
            input,
            output,
            running,
        } = resume_event()
        else {
            unreachable!()
        };
        app.apply_update(live::Update::Tool {
            id,
            name,
            input,
            output,
            running,
        });
        assert!(status(&app).is_none());
    }

    #[test]
    fn text_written_between_tool_calls_shows_between_them() {
        let mut app = live_app();
        app.submit("work", std::path::Path::new("."));
        let id = app.request.take().unwrap().id;
        let tool = |running| live::Update::Tool {
            id,
            name: "Run".into(),
            input: serde_json::json!({"command":"ls"}),
            output: serde_json::Value::Null,
            running,
        };
        app.apply_update(live::Update::Delta {
            id,
            text: "Looking first.".into(),
        });
        app.apply_update(tool(true));
        app.apply_update(tool(false));
        app.apply_update(live::Update::Delta {
            id,
            text: "\n\nDone.".into(),
        });
        app.apply_update(live::Update::Finished {
            id,
            result: Ok(openrouter::Streamed {
                text: "Looking first.\n\nDone.".into(),
                ..Default::default()
            }),
        });
        let shown: Vec<String> = app
            .live
            .entries
            .iter()
            .map(|entry| match entry {
                live::Entry::User(text) => format!("user {text}"),
                live::Entry::Assistant { text, .. } => format!("text {text}"),
                live::Entry::Tool { name, .. } => format!("tool {name}"),
                _ => "other".into(),
            })
            .collect();
        assert_eq!(
            shown,
            ["user work", "text Looking first.", "tool Run", "text Done."]
        );

        // A turn without tool calls still shows its one reply.
        app.submit("again", std::path::Path::new("."));
        let id = app.request.take().unwrap().id;
        app.apply_update(live::Update::Delta {
            id,
            text: "Hi.".into(),
        });
        app.apply_update(live::Update::Finished {
            id,
            result: Ok(openrouter::Streamed {
                text: "Hi.".into(),
                ..Default::default()
            }),
        });
        assert!(
            matches!(app.live.entries.last(), Some(live::Entry::Assistant { text, .. }) if text == "Hi.")
        );
        assert_eq!(app.live.entries.len(), 6);
    }

    #[test]
    fn the_status_line_adds_agent_dollars() {
        let mut app = live_app();
        app.live.cost_usd = 0.5;
        app.delegations.push(live::Delegation {
            id: "1:a".into(),
            name: "microcoder".into(),
            task: "fix".into(),
            chat: live::Chat {
                cost_usd: 0.25,
                ..live::Chat::default()
            },
            started_at: 0,
            elapsed_seconds: 0,
            running: false,
            draft: crate::Draft::default(),
            composer: Default::default(),
            scroll: 0,
            background: false,
        });
        assert_eq!(status(&app).as_deref(), Some("$0.75"));
        assert!((output_cost(&json!({"usage": {"cost": 0.125}})).unwrap() - 0.125).abs() < 1e-9);
    }

    #[test]
    fn clock_reads_hours_and_minutes() {
        let text = clock(atif::now_ms());
        assert_eq!(text.len(), 5);
        assert_eq!(&text[2..3], ":");
    }
}
