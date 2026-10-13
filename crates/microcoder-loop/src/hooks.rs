//! The Coder loop as a Coder SDK backend (#11497): the loop fires the
//! normalized hook events at its own points and obeys what they come to,
//! natively, since the host owns the loop.
//!
//! | Event | Where | Obeyed |
//! |---|---|---|
//! | `SessionStart` | before the first step | context joins the first step's notes |
//! | `UserPromptSubmit` | each message the person sends the running turn | a deny drops the message; context joins the notes |
//! | `PreToolUse` | before each command ([`TOOL`]) | deny or an unanswered ask: the command doesn't run and its output is the reason; a rewrite runs the rewritten command |
//! | `PostToolUse` | after each command the model chose | context joins the next step's notes |
//! | `PostToolBatch` | after a step's commands | context joins the next step's notes |
//! | `Stop` | where the loop would finish | a block keeps it working, with the reason as a note |
//! | `PostCompact` | after the step's prompt log started again compacted | context joins the next step's notes |
//! | `SessionEnd` | after the loop ends | observed only |
//!
//! Any of them may end the session: the loop stops as if stopped from
//! outside ([`crate::run::Ending::Stopped`]). Notes sit in the state
//! section at the end of the step's prompt, so what a hook adds never
//! breaks the cached prefix.
//!
//! The host gives the loop its hooks through [`crate::env::Env::hook`];
//! an environment with none answers every event unchanged.

use std::time::Duration;

use coder_sdk::{Applied, Call, Event, Permission, Stop};
use serde_json::{Value, json};

use crate::env::Env;
use crate::state::{CommandResult, State, cut};

/// The tool name a loop command fires as: the floors know it as Coder's
/// `Run` tool.
pub const TOOL: &str = "Run";

/// A command after its hooks.
pub struct Hooked {
    pub result: CommandResult,
    /// Context the hooks added, for the next step's notes.
    pub context: Vec<String>,
    /// A hook ended the session.
    pub end: bool,
}

/// Runs `command` through `PreToolUse`, and `PostToolUse` when `post`.
pub async fn run<E: Env>(env: &E, command: &str, deadline: Duration, post: bool) -> Hooked {
    let pre = env
        .hook(Call::tool(
            Event::PreToolUse,
            TOOL,
            json!({ "command": command }),
        ))
        .await;
    let mut context = pre.context_lines();
    let mut end = pre.stop == Stop::End;
    if let Permission::Deny { reason } | Permission::Ask { step: reason } = &pre.permission {
        return Hooked {
            result: CommandResult {
                command: command.to_owned(),
                exit: None,
                timed_out: false,
                seconds: 0.0,
                output: reason.clone(),
            },
            context,
            end,
        };
    }
    let command = pre.input["command"].as_str().unwrap_or(command).to_owned();
    let result = env.run(&command, deadline).await;
    if post && !end {
        let after = env
            .hook(Call::tool(
                Event::PostToolUse,
                TOOL,
                json!({
                    "tool_input": { "command": command },
                    "tool_response": response(&result),
                }),
            ))
            .await;
        context.extend(after.context_lines());
        end |= after.stop == Stop::End;
    }
    Hooked {
        result,
        context,
        end,
    }
}

/// What a hook reads of a command's result.
pub(crate) fn response(result: &CommandResult) -> Value {
    json!({
        "exit": result.exit,
        "timed_out": result.timed_out,
        "output": cut(&result.output, 4_000, 4_000),
    })
}

/// Fires `event` with `input`; its context joins `state`'s notes. Whether
/// a hook ended the session.
pub(crate) async fn notes<E: Env>(env: &E, state: &mut State, event: Event, input: Value) -> bool {
    let applied = env.hook(Call::new(event, input)).await;
    state.notes.extend(applied.context_lines());
    applied.stop == Stop::End
}

/// What the `Stop` hooks said where the loop would finish.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Stopping {
    Finish,
    /// Keep working: the reason joined the notes.
    Blocked,
    End,
}

/// Fires `Stop`. `active` says the loop already went on once because a
/// stop hook blocked it, as Claude Code's `stop_hook_active` does.
pub(crate) async fn stop<E: Env>(
    env: &E,
    state: &mut State,
    step: usize,
    active: bool,
) -> Stopping {
    let applied: Applied = env
        .hook(Call::new(
            Event::Stop,
            json!({ "step": step, "stop_hook_active": active }),
        ))
        .await;
    state.notes.extend(applied.context_lines());
    match applied.stop {
        Stop::End => Stopping::End,
        Stop::Block { reason } => {
            state.notes.push(reason);
            Stopping::Blocked
        }
        Stop::Continue => Stopping::Finish,
    }
}
