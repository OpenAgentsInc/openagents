//! A repository turn on Coder V1 (#10754): the studio seats' engine.
//!
//! A Claude or Codex route whose endpoint is [`CODER_V1_ENDPOINT`] hands the
//! turn to one Coder V1 session (`openagents coder chat --json`,
//! `crates/coder-new`) in the task's worktree, the way the workshop agent's
//! requests run (`coder::task::coder_v1`):
//!
//! - **Session**: one per task, `task-ID`, in the Coder store the owner's
//!   own Coder reads, so a later turn of the task continues it and the
//!   owner can open it with `/resume` or `coder --follow`.
//! - **Process**: the `openagents` program beside this one or on `PATH`,
//!   with the owner's login environment. Coder chooses its own provider:
//!   the Codex login, then Claude Code's, then the OpenAgents Gateway. Its
//!   Microcoder writes only the worktree and its own scratch, and a Codex
//!   delegation writes only the worktree, under Codex's sandbox. Full
//!   access only, as a lean session: the grant names this endpoint only
//!   then.
//! - **Transcript**: each command Coder ran and its reply go to the task's
//!   ATIF transcript as they arrive, bounded.
//! - **Cancellation**: a cancelled task interrupts Coder, which saves the
//!   session and exits.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use acp_client::StopReason;
use atif::{Call, Outcome as CallOutcome, Source, Step};
use coder::task::adapter::{Host, Route as GrantRoute};
use coder::task::coder_v1::{self, Ended as CoderEnded, Engine as _, Event};
use serde_json::{Value, json};

use super::devin::{Ended, Turn};
use super::lean_session::bounded;

pub use coder::task::capacity::CODER_V1_ENDPOINT;

/// The engine a Coder V1 turn records.
pub const ENGINE: &str = "coder-v1";
/// The effect kind and step extension it records.
pub const KIND: &str = "coder_v1";
/// The most bytes of one command's output the transcript keeps.
const TOOL_OUTPUT: usize = 16 * 1024;
/// The most bytes of the reply the transcript keeps.
const SEGMENT: usize = 256 * 1024;

/// Coder V1's `openagents` program for this host, or why there is none.
pub(crate) fn binary() -> Result<PathBuf, String> {
    coder_v1::cli_binary()
}

/// The session a task's turns share.
#[must_use]
pub fn session(task: &str) -> String {
    coder_v1::session_for(&format!("task-{task}")).replacen("agent-", "", 1)
}

/// Runs the turn as one Coder V1 turn in the task's session.
pub(crate) async fn turn(
    host: &Host,
    route: &GrantRoute,
    program: PathBuf,
    recipe: Option<&mut super::recipe::Recipe>,
) -> Turn {
    let mut ended = Ended {
        engine: ENGINE,
        agent: "Coder",
        ..Ended::default()
    };
    let session = session(host.task_id());
    let resumed = host.earlier_note(KIND).is_some();
    let prompt = super::recipe::agent_prompt(recipe.as_deref(), host, resumed);
    let env: Vec<(String, String)> = host
        .login_environment()
        .await
        .map(|login| {
            login
                .variables
                .iter()
                .filter_map(|(key, value)| {
                    Some((
                        key.clone().into_string().ok()?,
                        value.clone().into_string().ok()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let home = env
        .iter()
        .find(|(key, _)| key == "HOME")
        .map(|(_, value)| PathBuf::from(value))
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from));
    let state = home.map_or_else(
        || host.store().join("coder-new"),
        |home| home.join(".openagents/coder-new"),
    );
    let sequence = match host.effect(
        KIND,
        json!({"program": program, "cwd": host.workspace(), "session": session,
            "state": state, "route_model": route.model, "resume": resumed,
            "prompt_chars": prompt.chars().count()}),
    ) {
        Ok(sequence) => sequence,
        Err(error) => {
            ended.error = Some(error.to_string());
            return Turn::Ended(ended);
        }
    };
    // A workshop agent on Codex asks for the delegation in her brief. The
    // turn has full access to the task's own worktree, which its Microcoder
    // already writes, so her Codex delegation writes the same worktree under
    // Codex's sandbox, with no network. Other turns leave the flag off, so
    // they run on an `openagents` from before it.
    let codex_writes = prompt.contains(coder_v1::CODEX_DIRECTIVE);
    let turn = coder_v1::Turn {
        cwd: host.workspace().to_path_buf(),
        state,
        session: session.clone(),
        prompt,
        instructions: None,
        approvals: false,
        codex_writes,
        tool_free: false,
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let (events, heard) = std::sync::mpsc::channel::<Event>();
    let running = {
        let cancel = Arc::clone(&cancel);
        let mut cli = coder_v1::Cli { program, env };
        tokio::task::spawn_blocking(move || {
            cli.turn(&turn, &cancel, &mut |event| {
                let _ = events.send(event.clone());
                None
            })
        })
    };
    tokio::pin!(running);
    let mut tool_calls = 0;
    let mut model = None;
    let result = loop {
        tokio::select! {
            result = &mut running => break result,
            () = tokio::time::sleep(Duration::from_millis(250)) => {}
        }
        if host.cancelled() {
            cancel.store(true, Ordering::SeqCst);
        }
        for event in heard.try_iter() {
            hear(host, &event, &mut tool_calls, &mut model);
        }
    };
    for event in heard.try_iter() {
        hear(host, &event, &mut tool_calls, &mut model);
    }
    let coder_ended = result.unwrap_or_else(|error| CoderEnded::Failed(error.to_string()));
    ended.tool_calls = tool_calls;
    ended.session = Some(session.clone());
    ended.resumed = resumed;
    ended.model = model.or_else(|| Some(route.model.clone()));
    let observed = match &coder_ended {
        CoderEnded::Finished { reply, tokens } => {
            let reply = bounded(reply, SEGMENT);
            if !reply.trim().is_empty() {
                let _ = host.append(&Step::said(Source::Agent, &reply));
            }
            ended.reply = reply;
            ended.output_tokens = *tokens;
            ended.stop = Some(StopReason::EndTurn);
            json!({"status": "answered", "tokens": tokens})
        }
        CoderEnded::Failed(why) => {
            ended.error = Some(format!("Coder stopped: {why}"));
            json!({"status": "failed", "error": bounded(why, 4_000)})
        }
        CoderEnded::Cancelled => {
            ended.stop = Some(StopReason::Cancelled);
            json!({"status": "cancelled"})
        }
    };
    if let Err(error) = host.result(sequence, KIND, observed) {
        ended.error.get_or_insert(error.to_string());
    }
    let _ = host.append(
        &Step::said(Source::System, "The Coder V1 session this turn ran in.").noting(
            KIND,
            json!({"session": session, "model": ended.model, "resumed": resumed}),
        ),
    );
    Turn::Ended(ended)
}

/// What one Coder event leaves in the transcript.
fn hear(host: &Host, event: &Event, tool_calls: &mut usize, model: &mut Option<String>) {
    match event {
        Event::Model { model: name } if !name.is_empty() => *model = Some(name.clone()),
        Event::Tool {
            name,
            input,
            output,
            running: false,
            ..
        } if name == "Run" => {
            *tool_calls += 1;
            let command = input
                .as_str()
                .map_or_else(|| input.to_string(), str::to_owned);
            let exit = output.get("exit").and_then(Value::as_i64);
            let call = Call {
                id: format!("{KIND}-{tool_calls}"),
                name: "Bash".to_owned(),
                arguments: json!({"command": command}),
                output: bounded(
                    output.get("output").and_then(Value::as_str).unwrap_or(""),
                    TOOL_OUTPUT,
                ),
                outcome: if exit == Some(0) {
                    CallOutcome::Completed
                } else {
                    CallOutcome::Failed
                },
                milliseconds: 0,
                purpose: None,
                extra: serde_json::Map::new(),
            };
            let step = Step::called(call).noting(&format!("{KIND}_tool"), json!({"exit": exit}));
            if let Err(error) = host.append(&step) {
                host.fail(error.to_string());
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tasks_turns_share_one_coder_session() {
        assert_eq!(session("t-1a2b"), "task-t-1a2b");
        assert_eq!(session("t 1"), "task-t-1");
    }
}
