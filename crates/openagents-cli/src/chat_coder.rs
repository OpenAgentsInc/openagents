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
use openagents_chat::tool_groups::Stream;

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
        Kind::Host | Kind::Computer => "",
    }
}

/// One event: an NDJSON line, or the live view on stderr and the result on
/// stdout. The live view groups tool calls as every surface does
/// (#10117): `tools` holds what is not final yet.
pub(super) fn show(output: &Output, tools: &mut Stream, line: &Line) {
    if output.json() {
        if let Ok(text) = serde_json::to_string(line) {
            println!("{text}");
            let _ = std::io::stdout().flush();
        }
        return;
    }
    match tools.push(line.seq, &line.event) {
        Some(lines) => {
            for row in lines {
                eprintln!("  {row}");
            }
            return;
        }
        None => {
            for row in tools.flush() {
                eprintln!("  {row}");
            }
        }
    }
    match &line.event {
        CoderEvent::Result(result) => {
            // The live view's header only: stdout lists the files (#10331).
            if let Some(text) = coder_events::text(&line.event)
                && let Some(header) = text.lines().next()
            {
                eprintln!("{header}");
            }
            if !result.summary.trim().is_empty() {
                println!("{}", result.summary.trim());
            }
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
            if let Some(note) = location_note(result, line.thread.as_deref()) {
                eprintln!("{note}");
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

/// Where the completed change is saved, or how to apply unsaved work.
fn location_note(result: &coder_events::Finished, thread: Option<&str>) -> Option<String> {
    if let Some(issue) = &result.issue {
        Some(issue.line())
    } else if let Some(destination) = &result.pushed_to {
        Some(format!("The change is on {destination}."))
    } else if !result.files_changed.is_empty() {
        thread.map(|thread| format!("The change is in Coder's worktree. Apply it to your checkout with: openagents chat apply --thread {thread}"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pushed_result_replaces_the_apply_hint() {
        let mut result = coder_events::Finished {
            turn: 2,
            summary: "Committed and pushed.".into(),
            files_changed: vec![coder_events::FileChange {
                path: "calc.py".into(),
                ..Default::default()
            }],
            insertions: 1,
            deletions: 1,
            worktree: "/removed".into(),
            trajectory: "/trace".into(),
            issue: None,
            pushed_to: Some("origin/main".into()),
            cost_microusd: None,
        };
        assert_eq!(
            location_note(&result, Some("thread")).unwrap(),
            "The change is on origin/main."
        );
        result.pushed_to = None;
        assert!(
            location_note(&result, Some("thread"))
                .unwrap()
                .contains("openagents chat apply --thread thread")
        );
    }
}
