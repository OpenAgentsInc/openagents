//! One coding-agent CLI session briefed by Jev, in place of Microcoder's
//! step loop: the shared turn behind [`super::claude_session`] (#10246)
//! and [`super::codex_session`] (#10250).
//!
//! - **Briefing**: the delegate recipe's briefing, knowledge, and frozen
//!   checks ([`super::recipe`]) are the session's input, as on a whole
//!   agent; with the recipe off, the request alone.
//! - **Process**: the CLI (`claude -p` or `codex exec --json`), supervised
//!   in its own process group by `coder_delegate`'s CLI adapter, in the
//!   workspace, with the owner's login environment. Full access only.
//! - **Transcript**: commands, file changes, and the reply are appended to
//!   the ATIF transcript as they arrive, bounded.
//! - **Follow-ups**: a later turn of the same task resumes the session;
//!   one that can't resume starts afresh.
//! - **Cost**: the CLI's own cost (Claude Code's `total_cost_usd`, list
//!   price on a subscription) or the list-price estimate from the usage
//!   Codex reports, with the cache reads and writes in the turn's stats.
//! - **Capacity**: a session that ends on a usage or rate limit before it
//!   did any work is a refusal for the capacity book, as on the loop.
//! - **Cancellation**: a cancelled task stops the session's process.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::time::Duration;

use acp_client::StopReason;
use atif::{Call, Outcome as CallOutcome, Source, Step};
use coder::task::adapter::{Host, Route as GrantRoute};
use coder::task::capacity::{Kind, Provider, Refusal};
use coder_delegate::delegate::{Briefing, Credential, Status};
use coder_delegate::policy::{ExecutorHost, ExecutorPolicy};
use coder_delegate::stream::{Event, Kind as EventKind};
use serde_json::{Value, json};

use super::devin::{Ended, Turn};

/// What makes one engine's lean session its own.
pub(crate) struct Lean {
    /// The engine a turn records, such as `claude-code-session`.
    pub engine: &'static str,
    /// The agent in the transcript's own words, such as `Claude Code`.
    pub agent: &'static str,
    /// The effect kind, the step extension that names the session the
    /// next turn resumes, and the artifacts folder's stem.
    pub kind: &'static str,
    /// The recipe row whose effort applies.
    pub recipe_row: &'static str,
    /// The effort when neither the recipe nor the route names one.
    pub effort: &'static str,
    /// The provider a usage or rate limit is booked against.
    pub provider: Provider,
    /// The session's executor policy for a model at an effort.
    pub executor: fn(&str, &str) -> ExecutorPolicy,
    /// The credential the CLI will use, given the login's `HOME`.
    pub credential: fn(Option<&Path>) -> Credential,
    /// Codex settings passed as `-c key=value`; empty for Claude Code.
    pub codex_config: &'static [&'static str],
}

/// How often the host looks whether the task was cancelled.
const CANCEL_POLL: Duration = Duration::from_secs(2);
/// The most bytes of one command's output the transcript keeps.
const TOOL_OUTPUT: usize = 16 * 1024;
/// The most bytes of one reply the transcript keeps.
const SEGMENT: usize = 256 * 1024;

/// `text` cut to at most `limit` bytes on a character boundary.
pub(crate) fn bounded(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// The input-token parts of a `usage` object: uncached input, cache reads,
/// and cache writes.
pub(crate) fn input_parts(usage: Option<&Value>) -> (u64, u64, u64) {
    let read = |key: &str| {
        usage
            .and_then(|usage| usage.get(key))
            .and_then(Value::as_u64)
            .unwrap_or_default()
    };
    (
        read("input_tokens"),
        read("cache_read_input_tokens"),
        read("cache_creation_input_tokens"),
    )
}

/// What the session's events leave in the transcript.
struct Transcript<'a> {
    host: &'a Host,
    lean: &'a Lean,
    model: String,
    tool_calls: Cell<usize>,
    reply: RefCell<String>,
}

impl Transcript<'_> {
    fn append(&self, step: &Step) {
        if let Err(error) = self.host.append(step) {
            self.host.fail(error.to_string());
        }
    }

    fn hear(&self, event: &Event) {
        match &event.kind {
            EventKind::CommandCompleted {
                command,
                exit_code,
                output,
            } => {
                self.tool_calls.set(self.tool_calls.get() + 1);
                let call = Call {
                    id: format!("{}-{}", self.lean.kind, event.seq),
                    name: "Bash".to_owned(),
                    arguments: json!({"command": command}),
                    output: bounded(output, TOOL_OUTPUT),
                    outcome: if exit_code.is_none_or(|code| code == 0) {
                        CallOutcome::Completed
                    } else {
                        CallOutcome::Failed
                    },
                    milliseconds: 0,
                    purpose: None,
                    extra: serde_json::Map::new(),
                };
                self.append(&Step::called(call).by(&self.model).noting(
                    &format!("{}_tool", self.lean.kind),
                    json!({"exit_code": exit_code}),
                ));
            }
            EventKind::ArtifactChanged { path, change } => {
                self.tool_calls.set(self.tool_calls.get() + 1);
                self.append(
                    &Step::said(
                        Source::System,
                        &format!("{} changed {path} ({change}).", self.lean.agent),
                    )
                    .noting(
                        &format!("{}_artifact", self.lean.kind),
                        json!({"path": path, "change": change}),
                    ),
                );
            }
            EventKind::AssistantClaim { text } if !text.trim().is_empty() => {
                let text = bounded(text, SEGMENT);
                self.append(&Step::said(Source::Agent, &text).by(&self.model));
                *self.reply.borrow_mut() = text;
            }
            _ => {}
        }
    }
}

/// Runs the turn as one lean session of `lean`'s engine.
pub(crate) async fn turn(
    lean: &Lean,
    host: &Host,
    route: &GrantRoute,
    program: PathBuf,
    recipe: Option<&mut super::recipe::Recipe>,
) -> Turn {
    let mut ended = Ended {
        engine: lean.engine,
        agent: lean.agent,
        ..Ended::default()
    };
    let effort = recipe
        .as_deref()
        .and_then(|recipe| recipe.effort(lean.recipe_row, route.effort.as_deref()))
        .or_else(|| route.effort.clone())
        .unwrap_or_else(|| lean.effort.to_owned());
    let earlier = host.earlier_note(lean.kind).and_then(|note| {
        note.get("session")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    let artifacts = host
        .store()
        .join(lean.kind.replace('_', "-"))
        .join(host.task_id());
    if let Err(error) = std::fs::create_dir_all(&artifacts) {
        ended.error = Some(format!("cannot create {}: {error}", artifacts.display()));
        return Turn::Ended(ended);
    }
    let login = host.login_environment().await;
    let env: Vec<(String, String)> = login
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
    let credential = (lean.credential)(home.as_deref());
    let policy = (lean.executor)(&route.model, &effort);
    let mut cli = coder_delegate::policy::executor(
        &policy,
        ExecutorHost {
            binary: Some(program.clone()),
            credential,
            workdir: host.workspace().to_path_buf(),
            artifacts: artifacts.clone(),
            artifacts_label: artifacts.to_string_lossy().into_owned(),
            env,
        },
    );
    cli.codex_config = lean
        .codex_config
        .iter()
        .map(|&setting| setting.to_owned())
        .collect();

    let transcript = Transcript {
        host,
        lean,
        model: route.model.clone(),
        tool_calls: Cell::new(0),
        reply: RefCell::new(String::new()),
    };
    let mut resume = earlier;
    let mut attempts = 0;
    let report = loop {
        attempts += 1;
        let resumed = resume.is_some();
        let text = super::recipe::agent_prompt(recipe.as_deref(), host, resumed);
        let briefing = Briefing {
            text,
            cap: 0,
            included: Vec::new(),
            omitted: Vec::new(),
        };
        let sequence = match host.effect(
            lean.kind,
            json!({"program": program, "cwd": host.workspace(), "model": route.model,
                "effort": effort, "tools": policy.tools, "prompt_cache_ttl": policy.prompt_cache_ttl,
                "system": policy.system.as_ref().map(|system| &system.sections), "codex_config": lean.codex_config, "resume": resume, "briefing_chars": briefing.chars(),
                "briefing_sha256": briefing.sha256(), "artifacts": artifacts}),
        ) {
            Ok(sequence) => sequence,
            Err(error) => {
                ended.error = Some(error.to_string());
                return Turn::Ended(ended);
            }
        };
        let mut observer = |event: &Event| transcript.hear(event);
        let running = cli.execute_watched(&briefing, &Ok, resume.clone(), &mut observer);
        tokio::pin!(running);
        let cancelled = async {
            loop {
                tokio::time::sleep(CANCEL_POLL).await;
                if host.cancelled() {
                    return;
                }
            }
        };
        tokio::pin!(cancelled);
        // A cancelled task drops the session, which stops its process.
        let report = tokio::select! {
            report = &mut running => Some(report),
            () = &mut cancelled => None,
        };
        let Some(report) = report else {
            let _ = host.result(sequence, lean.kind, json!({"cancelled": true}));
            ended.stop = Some(StopReason::Cancelled);
            ended.resumed = resumed;
            break None;
        };
        let summary = &report.summary;
        let (uncached, cache_read, cache_write) = input_parts(summary.usage.as_ref());
        let observed = json!({"status": report.status.word(),
            "status_detail": report.status.to_string(), "session": summary.session_id,
            "model": summary.model, "version": summary.version, "num_turns": summary.num_turns,
            "api_calls": summary.api_calls, "cost_usd": summary.total_cost_usd,
            "usage": summary.usage, "input_tokens": uncached, "cache_read_input_tokens": cache_read,
            "cache_creation_input_tokens": cache_write, "milliseconds": report.milliseconds,
            "limit": summary.limit.as_ref().map(|limit| json!({"provider": limit.provider,
                "message": limit.message, "resets_at": limit.resets_at, "window": limit.window,
                "status": limit.status})),
            "stream": report.stream,
            "stderr_tail": if report.status == Status::Answered { Value::Null }
                else { json!(bounded(&report.stderr, 4_000)) }});
        if let Err(error) = host.result(sequence, lean.kind, observed) {
            ended.error = Some(error.to_string());
        }
        // A session that can't resume, such as one whose files are gone,
        // starts afresh, once.
        let lost = resumed
            && attempts == 1
            && report.status != Status::Answered
            && summary.result.as_deref().is_none_or(str::is_empty)
            && transcript.tool_calls.get() == 0;
        if lost {
            let _ = host.append(&Step::said(
                Source::System,
                &format!(
                    "{} couldn't resume the earlier session, so a new one starts.",
                    lean.agent
                ),
            ));
            resume = None;
            continue;
        }
        ended.resumed = resumed;
        break Some(report);
    };
    ended.tool_calls = transcript.tool_calls.get();
    ended.reply = transcript.reply.borrow().clone();
    let Some(report) = report else {
        return Turn::Ended(ended);
    };
    let summary = &report.summary;
    ended.session.clone_from(&summary.session_id);
    ended.model = summary.model.clone().or_else(|| Some(route.model.clone()));
    ended.cost_usd = summary.total_cost_usd;
    let (uncached, cache_read, cache_write) = input_parts(summary.usage.as_ref());
    ended.input_tokens = uncached + cache_read + cache_write;
    ended.output_tokens = summary
        .usage
        .as_ref()
        .and_then(|usage| usage.get("output_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    #[allow(clippy::cast_precision_loss)]
    {
        ended.stats.insert("input_uncached".into(), uncached as f64);
        ended
            .stats
            .insert("cache_read_input_tokens".into(), cache_read as f64);
        ended
            .stats
            .insert("cache_creation_input_tokens".into(), cache_write as f64);
        if let Some(turns) = summary.num_turns {
            ended.stats.insert("num_turns".into(), turns as f64);
        }
        if let Some(calls) = summary.api_calls {
            ended.stats.insert("api_calls".into(), calls as f64);
        }
    }
    if let Some(result) = summary.result.as_deref().filter(|r| !r.trim().is_empty()) {
        ended.reply = bounded(result, SEGMENT);
    }
    if let Some(session) = &summary.session_id {
        let _ = host.append(
            &Step::said(
                Source::System,
                &format!("The {} session this turn ran in.", lean.agent),
            )
            .noting(
                lean.kind,
                json!({"session": session, "model": ended.model, "resumed": ended.resumed}),
            ),
        );
    }
    match &report.status {
        Status::Answered => ended.stop = Some(StopReason::EndTurn),
        status => {
            // A usage or rate limit before any work is a refusal for the
            // capacity book, and the run moves to its next route.
            if let Some(limit) = &summary.limit
                && ended.tool_calls == 0
            {
                let now = coder::task::autostart::unix_now();
                let kind = if limit.status == Some(429) && limit.window.is_none() {
                    Kind::RateLimit
                } else {
                    Kind::UsageLimit
                };
                return Turn::Refused(Refusal::new(lean.provider, kind, now, limit.resets_at));
            }
            ended.error = Some(status.to_string());
        }
    }
    Turn::Ended(ended)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_splits_into_uncached_reads_and_writes() {
        let usage = json!({"input_tokens": 10, "cache_read_input_tokens": 900,
            "cache_creation_input_tokens": 90, "output_tokens": 5});
        assert_eq!(input_parts(Some(&usage)), (10, 900, 90));
        assert_eq!(input_parts(None), (0, 0, 0));
    }

    #[test]
    fn bounded_cuts_on_a_character_boundary() {
        assert_eq!(bounded("héllo", 2), "h…");
        assert_eq!(bounded("short", 10), "short");
    }
}
