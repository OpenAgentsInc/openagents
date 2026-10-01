//! How `openagents chat` prints a Coder run on this computer: a compact
//! live view on stderr and the result on stdout, or every event as one
//! NDJSON line under `--json`.
//!
//! Nothing here runs Coder or follows a task. The chat client
//! ([`openagents_chat::client`]) does, over
//! [`coder::task::chat_client::Here`], the shared start the host's
//! auto-start also uses, and the events are
//! [`openagents_chat::coder_events`], the stream the desktop and the phone
//! render.

use std::io::Write;

use openagents_chat::client::Kind;
use openagents_chat::coder_events::{self, CoderEvent, Line, StepKind};

use crate::out::Output;

/// The words that answer a thread's question from this terminal.
pub(super) fn answer_hint(kind: Kind, thread: &str) -> String {
    format!(
        "openagents chat answer{} --thread {thread} \"YOUR ANSWER\"",
        flag(kind)
    )
}

/// The switch that reaches the thread's store again: `--scratch` for a
/// scratch thread, `--local` for one in this command's own store (so a
/// host started later is not asked for it).
pub(super) fn flag(kind: Kind) -> &'static str {
    match kind {
        Kind::Scratch => " --scratch",
        Kind::InProcess => " --local",
        Kind::Host => "",
    }
}

/// One event: an NDJSON line, or the live view on stderr and the result on
/// stdout.
pub(super) fn show(output: &Output, line: &Line) {
    if output.json() {
        if let Ok(text) = serde_json::to_string(line) {
            println!("{text}");
            let _ = std::io::stdout().flush();
        }
        return;
    }
    match &line.event {
        CoderEvent::Result(result) => {
            if let Some(text) = coder_events::text(&line.event) {
                eprintln!("{text}");
            }
            println!("{}", result.summary.trim());
            for file in &result.files_changed {
                println!(
                    "  {} {} (+{} -{})",
                    file.status,
                    file.path,
                    file.added.map_or("?".into(), |n| n.to_string()),
                    file.removed.map_or("?".into(), |n| n.to_string())
                );
            }
            println!("worktree: {}", result.worktree);
            if let Some(issue) = &result.issue {
                println!("{}", issue.line());
            }
            let _ = std::io::stdout().flush();
        }
        CoderEvent::Question(asked) | CoderEvent::Approval(asked) => {
            println!("{}", asked.text.trim());
            if let Some(how) = &asked.answer {
                eprintln!("answer: {how}");
            }
        }
        CoderEvent::Step(step) if step.kind == StepKind::Reply => {}
        event => {
            if let Some(text) = coder_events::text(event) {
                eprintln!("{text}");
            }
        }
    }
}
