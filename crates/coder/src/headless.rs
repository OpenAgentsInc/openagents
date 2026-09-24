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
//!   standard error. `--json` puts the report on standard output as a
//!   stream instead — one object per event the turn reports, then the
//!   summary object — and what the terminal watches, the stream writes.
//! - **The trace is the same trace.** Headless mode calls
//!   [`coder::turn::run`], the same function the terminal calls, through
//!   the same [`Agent`], so an episode judged from a headless run is
//!   judging the agent rather than the harness.
//! - **The exit code carries the outcome.** Answered, declined, and failed
//!   are three different things, and a script should not have to read the
//!   reply to tell them apart.

use std::io::Write;

use coder::turn::{self, Completion, Event, Failure};
use coder::{Agent, Classified, Route, ShellEvent, Status};
use serde_json::{Value, json};

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
    let mut out = std::io::stdout();
    let named = options.trace.is_some();
    let opened = match &options.trace {
        Some(path) => Agent::recording_to(path),
        None => Agent::from_env(),
    };
    // A door the environment asks for two ways is a measurement of the
    // wrong thing, so it ends the run before a trace is opened rather than
    // after one has recorded which door it picked.
    let mut agent = match opened {
        Ok(agent) => agent.with_program_grant(options.programs.as_deref()),
        Err(why) => {
            return fail(
                &options,
                None,
                &Failure::host("config", why),
                false,
                &mut out,
            );
        }
    };
    // A caller that named a file is going to read it back, so a trace that
    // could not be opened ends the run rather than producing an
    // unrecorded turn that looks like a recorded one.
    if let Some(error) = agent.trace_error() {
        if named {
            return fail(
                &options,
                None,
                &Failure::host("trace", error),
                false,
                &mut out,
            );
        }
        eprintln!("not recording this turn: {error}");
    } else {
        match agent.trace_path() {
            Some(path) => eprintln!("recording this turn to {}", path.display()),
            None => eprintln!("not recording this turn: CODER_TRACE is off"),
        }
    }
    // Which door answers and why, before anything answers: a fallback
    // should never pass for the door the operator expected.
    eprintln!(
        "answering with {} ({}) because {}",
        agent.door(),
        agent.model(),
        agent.door_reason()
    );
    let trace = agent.trace_path().map(|path| path.display().to_string());

    report(&mut agent, &options, trace, &mut out).await
}

/// Runs the turn and writes what it reports, split from [`print`] so a
/// test can hold the stream without holding a process. The agent is the
/// same agent the terminal runs and the objects on the stream are the
/// same events it watches — not a second account of the run.
async fn report(
    agent: &mut Agent,
    options: &Print,
    trace: Option<String>,
    out: &mut (dyn Write + Send),
) -> u8 {
    let mut emitted = 0usize;
    // Standard error carries the commands, because a turn that runs
    // something on the machine should say so even when nobody is watching
    // a terminal. Under `--json` the same events also land on standard
    // output, one object each, ahead of the summary. Deltas join them only
    // when asked: the reply lands whole in the summary, and a reply's
    // worth of deltas is a flood a pipe should opt into.
    let mut events = |event: Event| {
        match &event {
            Event::Shell(ShellEvent::Proposed(proposal)) => eprintln!("$ {}", proposal.command),
            Event::Program(slug) => eprintln!("running program {slug}"),
            Event::Judgment(line) => eprintln!("  {line}"),
            _ => {}
        }
        if options.json
            && let Some(object) = event_object(&event, options.json_deltas)
        {
            emitted += 1;
            say(out, &object);
        }
    };
    let finished = turn::run(agent, options.prompt.clone(), &mut events).await;
    agent.finish_trace();

    match finished {
        Ok(finished) => {
            let code = match finished.completion {
                Completion::Answered => EXIT_OK,
                Completion::Declined | Completion::Refused => EXIT_DECLINED,
            };
            if options.json {
                let usage = finished.usage.map(|usage| {
                    json!({
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                    })
                });
                say(
                    out,
                    &json!({
                        "reply": finished.reply,
                        "trace": trace,
                        "outcome": finished.completion.word(),
                        "route": finished.route.as_ref().map(Route::word),
                        "program": finished.program.as_ref().and_then(|run| run.program.clone()),
                        "usage": usage,
                        "cost_usd": finished.cost_usd,
                        "error": Option::<String>::None,
                        "cause": Option::<String>::None,
                        "refusal": Option::<String>::None,
                        "events": emitted > 0,
                    }),
                );
            } else {
                say(out, &finished.reply);
                if let Some(usd) = finished.cost_usd {
                    eprintln!("this turn cost ${usd:.4}");
                }
            }
            code
        }
        Err(why) => fail(options, trace.as_deref(), &why, emitted > 0, out),
    }
}

/// Reports a turn that did not finish, in whichever shape the caller asked
/// for, and hands back the exit code.
///
/// `cause` and `refusal` ride along with the sentence because a relay that
/// would not take the job, a worker that never answered, and a worker that
/// declined are three states, and a harness that could only read the
/// sentence would be matching on prose to tell them apart. `events` is
/// whether event objects already landed on the stream, so a reader holding
/// only the last line can still tell a stream from a bare result.
fn fail(
    options: &Print,
    trace: Option<&str>,
    why: &Failure,
    events: bool,
    out: &mut dyn Write,
) -> u8 {
    if options.json {
        say(
            out,
            &json!({
                "reply": Option::<String>::None,
                "trace": trace,
                "outcome": "failed",
                "route": Option::<String>::None,
                "program": Option::<String>::None,
                "usage": Option::<String>::None,
                "cost_usd": Option::<f64>::None,
                "error": why.reason,
                "cause": why.cause,
                "refusal": why.refusal,
                "events": events,
            }),
        );
    } else {
        eprintln!("coder: {why}");
    }
    EXIT_FAILED
}

/// The object one turn event writes to a `--json` stream, or `None` when
/// the event is one the stream leaves out — a delta nobody asked for.
///
/// The object carries the values the terminal draws, under their own
/// names: a program's slug, a verdict's route and the answer behind it, a
/// command and what came back from it. A field the event does not have is
/// `null`, not a stand-in.
fn event_object(event: &Event, deltas: bool) -> Option<Value> {
    match event {
        Event::Program(slug) => Some(json!({
            "event": "program",
            "slug": slug,
        })),
        Event::Classified(classified) => Some(classified_object(classified)),
        Event::Judgment(line) => Some(json!({
            "event": "judgment",
            "line": line,
        })),
        Event::Shell(shell) => Some(shell_object(shell)),
        Event::Delta(text) => deltas.then(|| {
            json!({
                "event": "delta",
                "text": text,
            })
        }),
    }
}

/// The `classified` object: the verdict whole — the route the table made,
/// a halt's reason, and the `action` answer with its probabilities — or
/// the note saying classify did not run.
fn classified_object(classified: &Classified) -> Value {
    match classified {
        Classified::Judged(verdict) => {
            let halt = match &verdict.route {
                Route::Halt(why) => Some(why.clone()),
                _ => None,
            };
            let action = verdict.judgment.action.as_ref().map(|action| {
                json!({
                    "choice": action.choice,
                    "confidence": action.confidence,
                    "probabilities": action.probabilities,
                })
            });
            json!({
                "event": "classified",
                "route": verdict.route.word(),
                "halt": halt,
                "action": action,
                "note": Option::<String>::None,
            })
        }
        Classified::Skipped(note) => json!({
            "event": "classified",
            "route": Option::<String>::None,
            "halt": Option::<String>::None,
            "action": Value::Null,
            "note": note,
        }),
    }
}

/// The `shell_*` objects: a proposal's command and reason, an outcome's
/// status the way the trace words it, and the judge's line as it came.
fn shell_object(shell: &ShellEvent) -> Value {
    match shell {
        ShellEvent::Proposed(proposal) => json!({
            "event": "shell_proposed",
            "command": proposal.command,
            "why": proposal.why,
        }),
        ShellEvent::Ran(outcome) => {
            let exit_code = match outcome.status {
                Status::Exit(code) => Some(code),
                _ => None,
            };
            json!({
                "event": "shell_outcome",
                "command": outcome.proposal.command,
                "why": outcome.proposal.why,
                "status": outcome.status.to_string(),
                "exit_code": exit_code,
                "output": outcome.output,
                "bytes": outcome.bytes,
                "milliseconds": outcome.elapsed.as_millis() as u64,
            })
        }
        ShellEvent::Verdict(line) => json!({
            "event": "shell_verdict",
            "line": line,
        }),
    }
}

/// Writes one line to `out` and flushes it, so a caller reading the pipe
/// has each event as the turn reports it and the whole summary before the
/// process goes.
fn say(out: &mut dyn Write, what: &dyn std::fmt::Display) {
    let _ = writeln!(out, "{what}");
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder::generate::{Door, StubGenerate};
    use coder::{Judgment, Outcome, Proposal, Verdict};
    use indexmap::IndexMap;
    use std::time::Duration;

    /// `Print` as a harness spells it: JSON on, a stub door behind the
    /// agent, and no trace file to name.
    fn options(prompt: &str) -> Print {
        Print {
            prompt: prompt.to_string(),
            trace: None,
            json: true,
            json_deltas: false,
            programs: None,
        }
    }

    /// The stream's lines, each read back as the JSON it claims to be.
    fn objects(out: &[u8]) -> Vec<Value> {
        String::from_utf8(out.to_vec())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    /// `--json` writes every event as its own object ahead of the summary,
    /// and the summary keeps the fields it always had, marked as a stream.
    #[tokio::test]
    async fn json_streams_each_event_then_the_summary() {
        let mut agent = Agent::new(None, Door::Stub(StubGenerate::default()));
        let mut out = Vec::new();
        let code = report(&mut agent, &options("hello"), None, &mut out).await;

        assert_eq!(code, EXIT_OK);
        let lines = objects(&out);
        assert!(lines.len() >= 2, "a stream has events before the summary");
        let (events, summary) = lines.split_at(lines.len() - 1);
        for object in events {
            assert!(object["event"].is_string(), "no discriminator: {object}");
        }
        assert_eq!(events[0]["event"], "classified");
        assert_eq!(summary[0]["outcome"], "answered");
        assert_eq!(summary[0]["route"], "respond");
        assert_eq!(summary[0]["events"], true);
        for key in [
            "reply", "trace", "outcome", "route", "program", "usage", "error", "cause", "refusal",
            "events",
        ] {
            assert!(summary[0].get(key).is_some(), "{key} missing");
        }
    }

    /// Without `--json` nothing changes: standard output is the bare reply.
    #[tokio::test]
    async fn without_json_the_reply_is_the_whole_output() {
        let mut agent = Agent::new(None, Door::Stub(StubGenerate::default()));
        let mut out = Vec::new();
        let mut options = options("hello");
        options.json = false;
        let code = report(&mut agent, &options, None, &mut out).await;

        assert_eq!(code, EXIT_OK);
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(serde_json::from_str::<Value>(text.trim()).is_err());
    }

    /// Delta objects ride the stream only when the flag asks for them.
    #[tokio::test]
    async fn deltas_stream_only_when_asked() {
        for (deltas, wanted) in [(false, 0), (true, 1)] {
            let mut agent = Agent::new(None, Door::Stub(StubGenerate::default()));
            let mut out = Vec::new();
            let mut options = options("hello");
            options.json_deltas = deltas;
            report(&mut agent, &options, None, &mut out).await;

            let count = objects(&out)
                .iter()
                .filter(|object| object["event"] == "delta")
                .count();
            assert_eq!(count, wanted, "json_deltas={deltas}");
        }
    }

    /// A turn that finishes declined still wrote its classified event, and
    /// the summary still closes the stream and says so.
    #[tokio::test]
    async fn a_declined_turn_still_reports_its_classify() {
        let plan = r#"{"v":2,"commands":[{"command":"ls","why":"look"}]}"#;
        let mut agent = Agent::new(None, Door::Stub(StubGenerate::saying(plan)));
        let mut out = Vec::new();
        let code = report(
            &mut agent,
            &options("say something ambiguous"),
            None,
            &mut out,
        )
        .await;

        assert_eq!(code, EXIT_DECLINED);
        let lines = objects(&out);
        assert_eq!(lines[0]["event"], "classified");
        let summary = lines.last().unwrap();
        assert_eq!(summary["outcome"], "refused");
        assert_eq!(summary["events"], true);
    }

    /// A failure before the turn is a bare result: the object is the whole
    /// output and `events` says nothing came before it.
    #[test]
    fn a_failure_before_the_turn_is_a_bare_result() {
        let mut out = Vec::new();
        let code = fail(
            &options("hello"),
            None,
            &Failure::host("config", "two doors named"),
            false,
            &mut out,
        );

        assert_eq!(code, EXIT_FAILED);
        let lines = objects(&out);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["outcome"], "failed");
        assert_eq!(lines[0]["cause"], "config");
        assert_eq!(lines[0]["events"], false);
    }

    /// Each event becomes the object carrying the values the terminal
    /// draws, and nothing it does not have becomes a value.
    #[test]
    fn event_objects_carry_the_event() {
        let object = event_object(&Event::Program("burn-down".to_string()), false).unwrap();
        assert_eq!(object, json!({"event": "program", "slug": "burn-down"}));

        let mut probabilities = IndexMap::new();
        probabilities.insert("respond".to_string(), 0.9);
        probabilities.insert("none".to_string(), 0.1);
        let judged = Classified::Judged(Verdict {
            route: Route::Respond,
            judgment: Judgment {
                action: Some(jev::ChoiceAnswer {
                    choice: "respond".to_string(),
                    confidence: 0.9,
                    probabilities,
                }),
            },
        });
        let object = event_object(&Event::Classified(judged), false).unwrap();
        assert_eq!(object["event"], "classified");
        assert_eq!(object["route"], "respond");
        assert_eq!(object["halt"], Value::Null);
        assert_eq!(object["note"], Value::Null);
        assert_eq!(object["action"]["choice"], "respond");
        assert_eq!(object["action"]["confidence"], 0.9);
        assert_eq!(object["action"]["probabilities"]["respond"], 0.9);

        let skipped = Classified::Skipped(
            "no classifier is configured (no decision endpoint is set), so the reply is not routed"
                .to_string(),
        );
        let object = event_object(&Event::Classified(skipped), false).unwrap();
        assert_eq!(object["route"], Value::Null);
        assert_eq!(object["halt"], Value::Null);
        assert_eq!(object["action"], Value::Null);
        assert_eq!(
            object["note"],
            "no classifier is configured (no decision endpoint is set), so the reply is not routed"
        );

        let object = event_object(&Event::Judgment("stop 0.71".to_string()), false).unwrap();
        assert_eq!(object, json!({"event": "judgment", "line": "stop 0.71"}));

        let proposal = Proposal {
            command: "ls".to_string(),
            why: "look".to_string(),
        };
        let object =
            event_object(&Event::Shell(ShellEvent::Proposed(proposal.clone())), false).unwrap();
        assert_eq!(
            object,
            json!({"event": "shell_proposed", "command": "ls", "why": "look"})
        );

        let outcome = Outcome {
            proposal,
            status: Status::Exit(0),
            output: "a\nb\n".to_string(),
            bytes: 3,
            elapsed: Duration::from_millis(40),
        };
        let object = event_object(&Event::Shell(ShellEvent::Ran(outcome)), false).unwrap();
        assert_eq!(object["event"], "shell_outcome");
        assert_eq!(object["command"], "ls");
        assert_eq!(object["status"], "exit 0");
        assert_eq!(object["exit_code"], 0);
        assert_eq!(object["output"], "a\nb\n");
        assert_eq!(object["bytes"], 3);
        assert_eq!(object["milliseconds"], 40);

        let object = event_object(
            &Event::Shell(ShellEvent::Verdict("pass 0.91".to_string())),
            false,
        )
        .unwrap();
        assert_eq!(
            object,
            json!({"event": "shell_verdict", "line": "pass 0.91"})
        );

        assert!(event_object(&Event::Delta("hi".to_string()), false).is_none());
        assert_eq!(
            event_object(&Event::Delta("hi".to_string()), true).unwrap(),
            json!({"event": "delta", "text": "hi"})
        );
    }
}
