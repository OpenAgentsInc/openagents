//! A repository turn on a lean Claude Code session (#10246).
//!
//! The cost audit's 61% and 63% savings
//! (`docs/cost/2026-10-02-system-one-cost-efficiency-audit.md`) came from
//! Jev briefing one Claude Code session with lean settings, not from
//! Microcoder's step loop, whose fresh `claude -p` per step writes most of
//! its prompt to the cache and reads little back (#10209). A Claude route
//! whose endpoint is [`CLAUDE_SESSION_ENDPOINT`] runs that configuration:
//!
//! - **Briefing**: the delegate recipe's briefing, knowledge, and frozen
//!   checks ([`super::recipe`]) are the session's input, as on a whole
//!   agent; with the recipe off, the request alone.
//! - **Lean settings**: [`executor`]: Opus 5.5 (the route's model), six
//!   tools (`Bash, Read, Edit, Write, Glob, Grep`), the headless core
//!   system prompt in place of Claude Code's own, the five-minute prompt
//!   cache, no claude.ai connectors, and medium effort (low for a
//!   question), the recipe's `claude-session` row.
//! - **Process**: `claude -p --output-format stream-json`, supervised in its
//!   own process group by `coder_delegate`'s CLI adapter, in the workspace,
//!   with the owner's login environment. Full access only: the grant names
//!   this endpoint only then (`autostart::Engine::claude`).
//! - **Transcript**: commands, file changes, and the reply are appended to
//!   the ATIF transcript as they arrive, bounded.
//! - **Follow-ups**: a later turn of the same task resumes the session
//!   (`--resume`); one that can't resume starts afresh.
//! - **Cost**: Claude Code's own `total_cost_usd` (list price on a
//!   subscription), with the cache reads and writes in the turn's stats.
//! - **Capacity**: a session that ends on a usage or rate limit before it
//!   answered is a refusal for the capacity book, as on the loop.
//! - **Cancellation**: a cancelled task stops the session's process.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::time::Duration;

use acp_client::StopReason;
use atif::{Call, Outcome as CallOutcome, Source, Step};
use coder::task::adapter::{Host, Route as GrantRoute};
use coder::task::capacity::{Kind, Provider, Refusal};
use coder_delegate::delegate::{Briefing, Credential, Status};
use coder_delegate::policy::{AgentName, ExecutorHost, ExecutorPolicy, SessionPolicy};
use coder_delegate::stream::{Event, Kind as EventKind};
use serde_json::{Value, json};

use super::devin::{Ended, Turn};

pub use coder::task::capacity::CLAUDE_SESSION_ENDPOINT;

/// The step extension that names the Claude Code session a turn used,
/// which the next turn of the task resumes.
pub const SESSION_NOTE: &str = "claude_session";
/// The engine a lean-session turn records.
pub const ENGINE: &str = "claude-code-session";
/// The six tools the audit's lean session runs with.
pub const TOOLS: &str = "Bash,Read,Edit,Write,Glob,Grep";
/// The prompt-cache TTL: cache writes at 1.25x input instead of 2x.
pub const PROMPT_CACHE_TTL: &str = "5m";
/// The effort when neither the recipe nor the route names one.
pub const EFFORT: &str = "medium";
/// How often the host looks whether the task was cancelled.
const CANCEL_POLL: Duration = Duration::from_secs(2);
/// The most bytes of one command's output the transcript keeps.
const TOOL_OUTPUT: usize = 16 * 1024;
/// The most bytes of one reply the transcript keeps.
const SEGMENT: usize = 256 * 1024;

/// Claude Code for this host, or why there is none.
pub(crate) fn binary() -> Result<PathBuf, String> {
    if let Some(named) =
        std::env::var_os(microcoder_loop::claude::BIN_VAR).filter(|value| !value.is_empty())
    {
        let path = PathBuf::from(named);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!(
                "{} names {}, which isn't a file",
                microcoder_loop::claude::BIN_VAR,
                path.display()
            ))
        };
    }
    microcoder_loop::claude::locate().ok_or_else(|| {
        "no claude binary: set CLAUDE_BIN, or install Claude Code and run `claude` to log in"
            .to_owned()
    })
}

/// The lean session's executor policy for `model` at `effort`: the
/// audit's `matched-opus-medium-v8` executor, with no deadline but the
/// terminal turn's quiet guard.
#[must_use]
pub fn executor(model: &str, effort: &str) -> ExecutorPolicy {
    ExecutorPolicy {
        agent: AgentName::ClaudeCode,
        version: None,
        model: model.to_owned(),
        effort: Some(effort.to_owned()),
        tools: Some(TOOLS.to_owned()),
        prompt_cache_ttl: Some(PROMPT_CACHE_TTL.to_owned()),
        deadline_sec: coder_delegate::terminal::TURN_WALL.as_secs(),
        system: coder_delegate::system::Policy::preset("core"),
        session: Some(SessionPolicy {
            steer: None,
            stop_when: Some(coder_delegate::session::Trigger::Quiet {
                ms: u64::try_from(coder_delegate::terminal::TURN_QUIET.as_millis())
                    .unwrap_or(u64::MAX),
            }),
            resume: None,
        }),
        microluna: None,
    }
}

/// `text` cut to at most `limit` bytes on a character boundary.
fn bounded(text: &str, limit: usize) -> String {
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
fn input_parts(usage: Option<&Value>) -> (u64, u64, u64) {
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
                    id: format!("claude-{}", event.seq),
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
                self.append(
                    &Step::called(call)
                        .by(&self.model)
                        .noting("claude_session_tool", json!({"exit_code": exit_code})),
                );
            }
            EventKind::ArtifactChanged { path, change } => {
                self.tool_calls.set(self.tool_calls.get() + 1);
                self.append(
                    &Step::said(
                        Source::System,
                        &format!("Claude Code changed {path} ({change})."),
                    )
                    .noting(
                        "claude_session_artifact",
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

/// Runs the turn as one lean Claude Code session.
pub(crate) async fn turn(
    host: &Host,
    route: &GrantRoute,
    program: PathBuf,
    recipe: Option<&mut super::recipe::Recipe>,
) -> Turn {
    let mut ended = Ended {
        engine: ENGINE,
        agent: "Claude Code",
        ..Ended::default()
    };
    let effort = recipe
        .as_deref()
        .and_then(|recipe| {
            recipe.effort(
                route_contract::recipe::CLAUDE_SESSION,
                route.effort.as_deref(),
            )
        })
        .or_else(|| route.effort.clone())
        .unwrap_or_else(|| EFFORT.to_owned());
    let earlier = host.earlier_note(SESSION_NOTE).and_then(|note| {
        note.get("session")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    let artifacts = host.store().join("claude-session").join(host.task_id());
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
    // The loop's capacity probe already found a signed-in Claude Code; a
    // login kept in the system keychain has no credentials file to find.
    let credential = match Credential::detect(
        |name| std::env::var(name).ok(),
        coder_delegate::delegate::stored_login(home.as_deref()),
    ) {
        Credential::Missing => Credential::CliLogin,
        found => found,
    };
    let mut cli = coder_delegate::policy::executor(
        &executor(&route.model, &effort),
        ExecutorHost {
            binary: Some(program.clone()),
            credential,
            workdir: host.workspace().to_path_buf(),
            artifacts: artifacts.clone(),
            artifacts_label: artifacts.to_string_lossy().into_owned(),
            env,
        },
    );

    let transcript = Transcript {
        host,
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
            "claude_session",
            json!({"program": program, "cwd": host.workspace(), "model": route.model,
                "effort": effort, "tools": TOOLS, "prompt_cache_ttl": PROMPT_CACHE_TTL,
                "system": "core", "resume": resume, "briefing_chars": briefing.chars(),
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
            let _ = host.result(sequence, "claude_session", json!({"cancelled": true}));
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
        if let Err(error) = host.result(sequence, "claude_session", observed) {
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
                "Claude Code couldn't resume the earlier session, so a new one starts.",
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
            &Step::said(Source::System, "The Claude Code session this turn ran in.").noting(
                SESSION_NOTE,
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
                return Turn::Refused(Refusal::new(Provider::Claude, kind, now, limit.resets_at));
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
    fn the_executor_is_the_audits_lean_session() {
        let policy = executor("claude-opus-5-5", "medium");
        assert_eq!(policy.tools.as_deref(), Some(TOOLS));
        assert_eq!(policy.prompt_cache_ttl.as_deref(), Some("5m"));
        assert_eq!(policy.effort.as_deref(), Some("medium"));
        let system = policy.system.clone().expect("a trimmed system prompt");
        assert_eq!(system.mode, coder_delegate::system::Mode::Replace);
        let cli = coder_delegate::policy::executor(
            &policy,
            ExecutorHost {
                binary: None,
                credential: Credential::CliLogin,
                workdir: PathBuf::from("/w"),
                artifacts: PathBuf::from("/a"),
                artifacts_label: "a".into(),
                env: Vec::new(),
            },
        );
        assert_eq!(cli.agent, coder_delegate::delegate::Agent::ClaudeCode);
        assert_eq!(cli.model, "claude-opus-5-5");
        assert!(cli.system.is_some());
    }

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
