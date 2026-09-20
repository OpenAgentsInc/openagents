//! `coder --print`: one turn, no terminal.
//!
//! The terminal is a fine way to talk to the agent and a useless way to
//! measure it. Headless mode reads a prompt, runs the same turn the
//! terminal runs, writes the reply to standard output, and exits with a
//! code that says how the turn went. That is what a script needs, and it
//! is what lets an episode be run and judged rather than watched.
//!
//! Three things the mode owes its caller:
//!
//! - **Standard output is the reply and nothing else.** Progress goes to
//!   standard error, and `--json` puts the whole report on standard output
//!   as one object instead.
//! - **The trace is the same trace.** Headless mode calls
//!   [`coder::turn::run`], the same function the terminal calls, through
//!   the same [`Agent`], so an episode judged from a headless run is
//!   judging the agent rather than the harness.
//! - **The exit code carries the outcome.** Answered, declined, and failed
//!   are three different things, and a script should not have to read the
//!   reply to tell them apart.

use std::io::Write;

use coder::turn::{self, Completion, Event};
use coder::{Agent, ShellEvent};
use serde_json::json;

use crate::cli::Print;

/// The turn finished and the agent answered.
pub const EXIT_OK: u8 = 0;

/// The turn did not finish. The door failed, or a named trace could not be
/// opened, and there is no reply.
pub const EXIT_FAILED: u8 = 1;

/// The turn finished and the router declined it. Nothing went wrong; the
/// answer is that there is no confident next step.
pub const EXIT_DECLINED: u8 = 2;

/// Runs one turn and reports it. The returned code is the process's.
pub async fn print(options: Print) -> u8 {
    let named = options.trace.is_some();
    let mut agent = match &options.trace {
        Some(path) => Agent::recording_to(path),
        None => Agent::from_env(),
    };
    // A caller that named a file is going to read it back, so a trace that
    // could not be opened ends the run rather than producing an
    // unrecorded turn that looks like a recorded one.
    if let Some(error) = agent.trace_error() {
        if named {
            return fail(&options, None, error);
        }
        eprintln!("no trace — {error}");
    } else {
        match agent.trace_path() {
            Some(path) => eprintln!("trace → {}", path.display()),
            None => eprintln!("no trace — CODER_TRACE is off"),
        }
    }
    let trace = agent.trace_path().map(|path| path.display().to_string());

    // Standard error carries the commands, because a turn that runs
    // something on the machine should say so even when nobody is watching
    // a terminal. Deltas are dropped: the reply lands whole below, and a
    // plan streaming to standard output would corrupt it.
    let mut events = |event: Event| {
        if let Event::Shell(ShellEvent::Proposed(proposal)) = event {
            eprintln!("$ {}", proposal.command);
        }
    };
    let finished = turn::run(&mut agent, options.prompt.clone(), &mut events).await;
    agent.finish_trace();

    match finished {
        Ok(finished) => {
            let code = match finished.completion {
                Completion::Answered => EXIT_OK,
                Completion::Declined => EXIT_DECLINED,
            };
            if options.json {
                let usage = finished.usage.map(|usage| {
                    json!({
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                    })
                });
                say(&json!({
                    "reply": finished.reply,
                    "trace": trace,
                    "outcome": finished.completion.word(),
                    "route": finished.route.word(),
                    "usage": usage,
                    "error": Option::<String>::None,
                }));
            } else {
                say(&finished.reply);
            }
            code
        }
        Err(why) => fail(&options, trace.as_deref(), &why),
    }
}

/// Reports a turn that did not finish, in whichever shape the caller asked
/// for, and hands back the exit code.
fn fail(options: &Print, trace: Option<&str>, why: &str) -> u8 {
    if options.json {
        say(&json!({
            "reply": Option::<String>::None,
            "trace": trace,
            "outcome": "failed",
            "route": Option::<String>::None,
            "usage": Option::<String>::None,
            "error": why,
        }));
    } else {
        eprintln!("coder: {why}");
    }
    EXIT_FAILED
}

/// Writes one line to standard output and flushes it, so a caller reading
/// the pipe has the whole thing before the process goes.
fn say(what: &dyn std::fmt::Display) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{what}");
    let _ = out.flush();
}
