//! The replay (#11491's "done when"): the route this hook picks for every
//! later message of recorded Claude Code chats, and the tokens it would
//! have saved.
//!
//! The chats are Coder's saved sessions (`<data>/sessions/*.atif.json`)
//! that ran on Claude Code and hold more than one message from the person.
//! Each later message goes through the same [`super::rule`], question and
//! [`super::act`] as the live hook, against the real Decisions API. The
//! tokens come from Claude Code's own session file
//! (`~/.claude/projects/*/<session>.jsonl`): each model call's input,
//! cache-read and cache-write tokens, grouped by the person's prompts.
//!
//! What a route saves, estimated:
//!
//! - **host answer**: every prompt token of that turn's calls (the executor
//!   never runs);
//! - **re-brief**: from that turn on, each call's history beyond a fresh
//!   session's opening size (the session's first call), until the next
//!   re-brief. Those tokens are mostly cache reads, so the billed figure
//!   weighs them at a tenth; the fresh session's one cache write is not
//!   subtracted.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use coder_sdk::decide::Decider;
use coder_sdk::{Acted, Ruled};
use jev::decisions::DecisionsRequest;
use serde_json::Value;

use super::{CONTINUE, Facts, HOST_MODEL, HOST_SPEND, Note, REBRIEF};
use crate::router::Said;

/// One model call's tokens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Call {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Call {
    #[must_use]
    pub fn prompt(&self) -> u64 {
        self.input + self.cache_read + self.cache_write
    }

    /// The prompt in uncached-input equivalents: cache reads at a tenth,
    /// writes at one and a quarter.
    #[must_use]
    pub fn billed(&self) -> f64 {
        self.input as f64 + self.cache_read as f64 * 0.1 + self.cache_write as f64 * 1.25
    }
}

/// One prompt in a Claude Code session file and the calls it led to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prompt {
    pub text: String,
    pub calls: Vec<Call>,
}

/// Reads a Claude Code session file: the person's prompts, each with its
/// calls (one per assistant message id).
#[must_use]
pub fn prompts(jsonl: &str) -> Vec<Prompt> {
    let mut out: Vec<Prompt> = Vec::new();
    let mut seen = HashSet::new();
    for line in jsonl.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match entry["type"].as_str() {
            Some("user") if entry["isMeta"] != true => {
                let content = &entry["message"]["content"];
                let text = match content {
                    Value::String(text) => Some(text.clone()),
                    Value::Array(blocks)
                        if blocks.iter().all(|block| block["type"] != "tool_result") =>
                    {
                        Some(
                            blocks
                                .iter()
                                .filter_map(|block| block["text"].as_str())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        )
                    }
                    _ => None,
                };
                if let Some(text) = text {
                    out.push(Prompt {
                        text,
                        calls: Vec::new(),
                    });
                }
            }
            Some("assistant") => {
                let message = &entry["message"];
                let id = message["id"].as_str().unwrap_or_default().to_owned();
                if id.is_empty() || !seen.insert(id) {
                    continue;
                }
                let usage = &message["usage"];
                let call = Call {
                    input: usage["input_tokens"].as_u64().unwrap_or(0),
                    cache_read: usage["cache_read_input_tokens"].as_u64().unwrap_or(0),
                    cache_write: usage["cache_creation_input_tokens"].as_u64().unwrap_or(0),
                };
                if let Some(prompt) = out.last_mut() {
                    prompt.calls.push(call);
                }
            }
            _ => {}
        }
    }
    out
}

/// One recorded chat.
#[derive(Clone, Debug, Default)]
pub struct Chat {
    pub id: String,
    /// The person's messages and the non-empty replies, in order.
    pub said: Vec<Said>,
    /// Claude Code's prompts for the chat's session, when its file is here.
    pub prompts: Vec<Prompt>,
    pub model: Option<String>,
}

/// Reads one saved session, if it ran on Claude Code with a later message.
#[must_use]
pub fn chat(document: &Value, claude_projects: &Path) -> Option<Chat> {
    if document["extra"]["runs_on"] != "claude-code" {
        return None;
    }
    let mut said = Vec::new();
    for step in document["steps"].as_array()? {
        let text = match &step["message"] {
            Value::String(text) => text.clone(),
            other => other.as_str().unwrap_or_default().to_owned(),
        };
        if text.trim().is_empty() {
            continue;
        }
        let role = match step["source"].as_str() {
            Some("user") => "user",
            Some("agent") => "assistant",
            _ => continue,
        };
        said.push(Said {
            role: role.to_owned(),
            text,
        });
    }
    if said.iter().filter(|s| s.role == "user").count() < 2 {
        return None;
    }
    let session = document["extra"]["claude_session"]["id"].as_str();
    let mut prompts_found = Vec::new();
    let mut model = None;
    if let Some(session) = session
        && let Some(file) = find_session(claude_projects, session)
        && let Ok(text) = std::fs::read_to_string(file)
    {
        prompts_found = prompts(&text);
        model = text.lines().find_map(|line| {
            let entry: Value = serde_json::from_str(line).ok()?;
            (entry["type"] == "assistant")
                .then(|| entry["message"]["model"].as_str().map(crate::claude::name))
                .flatten()
        });
    }
    Some(Chat {
        id: document["session_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        said,
        prompts: prompts_found,
        model,
    })
}

fn find_session(projects: &Path, id: &str) -> Option<PathBuf> {
    let file = format!("{id}.jsonl");
    std::fs::read_dir(projects)
        .ok()?
        .flatten()
        .map(|project| project.path().join(&file))
        .find(|path| path.is_file())
}

/// Every replayable chat under `sessions`.
#[must_use]
pub fn chats(sessions: &Path, claude_projects: &Path) -> Vec<Chat> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(sessions)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".atif.json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|text| serde_json::from_str::<Value>(&text).ok())
        .filter_map(|document| chat(&document, claude_projects))
        .collect()
}

/// The tally.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub chats: usize,
    pub messages: usize,
    /// Routes by name, `pass` for messages the rule settled, `failed` for a
    /// call that didn't answer.
    pub routes: std::collections::BTreeMap<String, usize>,
    /// Prompt tokens of the later turns that have a token record.
    pub later_tokens: u64,
    pub later_billed: f64,
    pub saved_by_host: u64,
    pub saved_by_rebrief: u64,
    pub saved_billed: f64,
    /// Each routed message: chat, route, p, the message's head.
    pub examples: Vec<(String, String, f64, String)>,
}

/// The route for one later message, through the hook's own rule, question
/// and act.
fn route(note: Note, decider: &dyn Decider) -> Result<(Acted, Value), String> {
    let latest = note.latest.clone();
    let Ruled::Unsure {
        input,
        questions,
        memo,
    } = super::rule(Some(note), &latest)
    else {
        return Err("pass".to_owned());
    };
    let mut request = DecisionsRequest::new(coder_sdk::decision::MODEL, input)
        .metadata("hook", super::HOOK)
        .metadata("question_set", super::QUESTION_SET);
    request.questions = questions;
    let response = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?
        .block_on(decider.decide(request))?;
    Ok((super::act(&memo, &response), memo))
}

/// The prompt in `prompts` (from `from` on) that carries `message`.
fn aligned(prompts: &[Prompt], from: usize, message: &str) -> Option<usize> {
    let head: String = message.trim().chars().take(120).collect();
    if head.is_empty() {
        return None;
    }
    (from..prompts.len()).find(|&i| prompts[i].text.contains(&head))
}

/// Replays `chats` through `decider`.
#[must_use]
pub fn replay(chats: &[Chat], decider: &dyn Decider) -> Report {
    let mut report = Report {
        chats: chats.len(),
        ..Report::default()
    };
    for chat in chats {
        // The route of each Claude Code prompt this chat's messages map to.
        let mut by_prompt: Vec<Option<String>> = vec![None; chat.prompts.len()];
        let mut next_prompt = 0;
        let mut spent: u64 = 0;
        let users: Vec<usize> = (0..chat.said.len())
            .filter(|&i| chat.said[i].role == "user")
            .collect();
        for (n, &i) in users.iter().enumerate() {
            let message = &chat.said[i].text;
            let at = aligned(&chat.prompts, next_prompt, message);
            if let Some(at) = at {
                spent += chat.prompts[next_prompt..at]
                    .iter()
                    .flat_map(|p| &p.calls)
                    .map(Call::prompt)
                    .sum::<u64>();
                next_prompt = at + 1;
            }
            if n == 0 {
                if let Some(at) = at {
                    spent += chat.prompts[at].calls.iter().map(Call::prompt).sum::<u64>();
                }
                continue;
            }
            report.messages += 1;
            // A message that opened a Claude Code session is the pasted
            // conversation of a new process, which the live hook never
            // routes.
            if at == Some(0) || aligned(&chat.prompts, 0, message) == Some(0) {
                *report.routes.entry("pass".to_owned()).or_default() += 1;
                continue;
            }
            let note = Note {
                earlier: chat.said[..i].to_vec(),
                latest: message.clone(),
                midturn: false,
                facts: Facts {
                    model: Some(chat.model.clone().unwrap_or_else(|| "Claude".to_owned())),
                    spend: Some((spent, 0.0)),
                },
            };
            let name = match route(note, decider) {
                Ok((acted, _)) => {
                    if acted.action != CONTINUE {
                        report.examples.push((
                            chat.id.clone(),
                            acted.action.clone(),
                            acted.p.unwrap_or(0.0),
                            message.chars().take(100).collect(),
                        ));
                    }
                    acted.action
                }
                Err(error) if error == "pass" => "pass".to_owned(),
                Err(_) => "failed".to_owned(),
            };
            *report.routes.entry(name.clone()).or_default() += 1;
            if let Some(at) = at {
                by_prompt[at] = Some(name);
                spent += chat.prompts[at].calls.iter().map(Call::prompt).sum::<u64>();
            }
        }
        tally_tokens(chat, &by_prompt, &mut report);
    }
    report
}

/// Adds `chat`'s later-turn tokens and the estimated savings of its routes.
fn tally_tokens(chat: &Chat, by_prompt: &[Option<String>], report: &mut Report) {
    let Some(base) = chat
        .prompts
        .first()
        .and_then(|p| p.calls.first())
        .map(Call::prompt)
    else {
        return;
    };
    // Tokens a fresh session from the latest re-brief would not carry.
    let mut offset: u64 = 0;
    for (k, prompt) in chat.prompts.iter().enumerate().skip(1) {
        let route = by_prompt[k].as_deref();
        for call in &prompt.calls {
            report.later_tokens += call.prompt();
            report.later_billed += call.billed();
        }
        match route {
            Some(HOST_MODEL | HOST_SPEND) => {
                for call in &prompt.calls {
                    report.saved_by_host += call.prompt();
                    report.saved_billed += call.billed();
                }
                continue;
            }
            Some(REBRIEF) => {
                offset = prompt
                    .calls
                    .first()
                    .map_or(0, |call| call.prompt().saturating_sub(base));
            }
            _ => {}
        }
        for call in &prompt.calls {
            let saved = offset.min(call.prompt());
            report.saved_by_rebrief += saved;
            report.saved_billed += saved as f64 * 0.1;
        }
    }
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} chats, {} later messages", self.chats, self.messages)?;
        for (route, count) in &self.routes {
            let share = *count as f64 * 100.0 / self.messages.max(1) as f64;
            writeln!(f, "  {route:<12} {count:>4}  {share:>5.1}%")?;
        }
        let pct = |n: f64, of: f64| if of > 0.0 { n * 100.0 / of } else { 0.0 };
        writeln!(
            f,
            "later-turn prompt tokens: {} ({:.0} billed-equivalent)",
            self.later_tokens, self.later_billed
        )?;
        writeln!(
            f,
            "saved by host answers: {} ({:.2}%)",
            self.saved_by_host,
            pct(self.saved_by_host as f64, self.later_tokens as f64)
        )?;
        writeln!(
            f,
            "saved by re-briefs:    {} ({:.2}%)",
            self.saved_by_rebrief,
            pct(self.saved_by_rebrief as f64, self.later_tokens as f64)
        )?;
        writeln!(
            f,
            "saved, billed-equivalent: {:.0} ({:.2}%)",
            self.saved_billed,
            pct(self.saved_billed, self.later_billed)
        )?;
        for (chat, route, p, head) in &self.examples {
            writeln!(f, "  {route} p={p:.2} [{chat}] {head}")?;
        }
        Ok(())
    }
}
