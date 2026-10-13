//! Solver as a plugin: the live model hands "solve issue N to a pull
//! request" to [`crate::solver`], the same four-hand run as `coder solver
//! N`, and shows it as a delegation.
//!
//! The run is started in this process ([`crate::solver::start`]) rather than
//! as a child `coder solver --plain --json`: the feed it returns is the same
//! stream of transcript changes that command prints, so nothing is parsed
//! twice and the run never depends on where the binary lives. Each change
//! becomes a [`RuntimeEvent`] in the delegation's child chat ([`Mapper`]);
//! the run's Summary becomes the tool's short result (outcome, pull
//! request, cost, run folder). Stopping the chat stops the run.
//!
//! The plugin is on by default and is turned off in /plugins like any other
//! (`bundled-plugins.json`, `solver`); when it is off the tool is not
//! offered and a call is refused.

use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::bundled_runtime::RuntimeEvent;
use crate::issue_run::{self, Event};
use crate::live::Entry;

/// The plugin's ID in /plugins and in `bundled-plugins.json`.
pub const PLUGIN: &str = "solver";

/// The tool the model calls.
pub const TOOL: &str = "solver";

/// The delegation's name where a person reads it.
pub const NAME: &str = "Solver";

/// A recording the host plays instead of starting a run, for demos and
/// tests of the whole delegation path without spending a run. Never set
/// by the model.
pub const REPLAY_ENV: &str = "CODER_SOLVER_REPLAY";

/// How long a stopped run is given to write its summary before the
/// delegation returns.
const STOP_GRACE: Duration = Duration::from_secs(20);

const DESCRIPTION: &str = "Hand one GitHub issue to Solver, which takes it to a pull request: it reads the issue, works out what the change should be, writes the code, runs the checks, and opens the pull request. Use it when the person asks to solve, fix, or implement an issue by its number. `issue` is N, #N, or owner/repo#N; without a repository it is an issue of the repository the chat's folder is in (its GitHub remote); when that folder has none, ask the person which repository in one line. Set `no_pr` to true to stop after the checks without opening a pull request. A run takes minutes. It returns the outcome, the pull request link, the cost, and the folder with the run's files.";

/// The tool's definition, in the chat-completions function shape.
#[must_use]
pub fn tool_definition() -> Value {
    json!({"type":"function","function":{
        "name": TOOL,
        "description": DESCRIPTION,
        "parameters": schema(),
    }})
}

/// The tool's input schema.
#[must_use]
pub fn schema() -> Value {
    json!({
        "type":"object",
        "properties":{
            "issue":{"type":["string","integer"],"description":"N, #N, or owner/repo#N."},
            "no_pr":{"type":"boolean","description":"Stop after the checks; open no pull request."}
        },
        "required":["issue"],
        "additionalProperties":false
    })
}

/// What the model is told about the tool.
#[must_use]
pub const fn instructions() -> &'static str {
    "Solver takes one GitHub issue to a pull request in its own run (reading the issue, planning the change, coding, checking). When the person asks to solve, fix, or implement an issue by number, call the solver tool with that issue rather than doing the work yourself; pass no_pr true when they ask for no pull request. Report its outcome, pull request link, cost, and run folder in a sentence or two.\n"
}

/// The issue a call names: `(repository, number)`.
///
/// # Errors
/// The arguments are not `{issue, no_pr?}` or the issue is not N, #N, or
/// owner/repo#N.
pub fn arguments(arguments: &Value) -> Result<(Option<String>, u64, bool), String> {
    const USAGE: &str =
        "solver takes issue (N, #N, or owner/repo#N) and optionally no_pr, with no other fields.";
    let object = arguments.as_object().ok_or(USAGE)?;
    if object.keys().any(|key| key != "issue" && key != "no_pr") {
        return Err(USAGE.into());
    }
    let no_pr = match object.get("no_pr") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err(USAGE.into()),
    };
    let text = match object.get("issue") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        _ => return Err(USAGE.into()),
    };
    let (repo, issue) = crate::solver::issue_reference(&text).ok_or(USAGE)?;
    Ok((repo, issue, no_pr))
}

/// The delegation's task line: what the child chat opens with.
#[must_use]
pub fn task(arguments: &Value) -> Option<String> {
    let (repo, issue, no_pr) = self::arguments(arguments).ok()?;
    // A bare number is the chat folder's repository, resolved by the run.
    let repo = repo.unwrap_or_default();
    Some(if no_pr {
        format!("Solve {repo}#{issue}, without opening a pull request.")
    } else {
        format!("Solve {repo}#{issue} to a pull request.")
    })
}

/// The run's options for a call made in `cwd`. A bare number is an issue
/// of `cwd`'s GitHub repository (#11536).
///
/// # Errors
/// The arguments are invalid, or a bare number was given in a folder with
/// no GitHub remote (the error asks which repository).
pub fn options(arguments: &Value, cwd: &Path) -> Result<crate::solver::Options, String> {
    let (repo, issue, no_pr) = self::arguments(arguments)?;
    let mut args = vec![
        issue.to_string(),
        "--repo".into(),
        cwd.display().to_string(),
    ];
    if let Some(repo) = repo {
        args.extend(["--github".into(), repo]);
    }
    if no_pr {
        args.push("--no-pr".into());
    }
    #[cfg(test)]
    let test_replay = tests::REPLAY.with(|replay| replay.borrow().clone());
    #[cfg(not(test))]
    let test_replay: Option<PathBuf> = None;
    let replay = test_replay.or_else(|| {
        std::env::var_os(REPLAY_ENV)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    });
    if let Some(replay) = replay {
        args.extend([
            "--replay".into(),
            replay.display().to_string(),
            "--speed".into(),
            "50".into(),
        ]);
    }
    crate::solver::Options::parse(&args)
}

/// Runs the solver on `arguments` in `cwd` until it ends or `cancel` is
/// set, sending its progress to `emit`.
///
/// # Errors
/// The arguments are invalid, the run was stopped, or it ended without a
/// result.
pub async fn execute(
    arguments: &Value,
    execution: &crate::plugin_tools::ExecutionSettings,
    cancel: &AtomicBool,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<Value, String> {
    execute_in(
        arguments,
        &execution.cwd,
        execution.claude,
        execution.connections.clone(),
        cancel,
        emit,
    )
    .await
}

/// [`execute`] in `cwd`, with the Claude Code plugin `claude` and the
/// chat's `account`. With the plugin off the hands run on the Coder loop
/// through the gateway, on this chat's sign-in (#11518).
///
/// # Errors
/// As [`execute`].
pub async fn execute_in(
    arguments: &Value,
    cwd: &Path,
    claude: bool,
    account: Option<openagents_login::Saved>,
    cancel: &AtomicBool,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<Value, String> {
    let mut options = options(arguments, cwd)?;
    // Without Claude Code, the next harness in the order (#11533).
    if !claude && options.engine == crate::solver::Engine::Claude {
        let opencode_on = crate::home::data_dir().is_none_or(|dir| {
            crate::bundled_settings::saved_enabled(&dir, crate::opencode_chat::PLUGIN)
        });
        if crate::solver::Engine::opencode_here(opencode_on) {
            options.use_opencode();
        } else {
            options.use_coder();
        }
    }
    if account.is_some() {
        options.inner.account = account;
    }
    run(crate::solver::start(options), cancel, emit).await
}

/// Whether Solver has an engine it can run on in this chat: Claude Code
/// while its plugin is on, or the Coder loop on a signed-in gateway
/// (#11518). With neither, the tool isn't offered.
#[must_use]
pub fn runnable(execution: &crate::plugin_tools::ExecutionSettings) -> bool {
    execution.claude || execution.connections.is_some()
}

/// Follows a started run's `feed` to its end.
///
/// # Errors
/// The run was stopped or ended without a result.
pub async fn run(
    feed: issue_run::Feed,
    cancel: &AtomicBool,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<Value, String> {
    let mut mapper = Mapper::default();
    let mut stopped: Option<Instant> = None;
    loop {
        let (events, ended) = feed.poll();
        for event in events {
            mapper.apply(event, emit);
        }
        if ended {
            break;
        }
        if stopped.is_none() && cancel.load(Ordering::Relaxed) {
            feed.stop();
            stopped = Some(Instant::now());
        }
        if stopped.is_some_and(|at| at.elapsed() > STOP_GRACE) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    drop(feed);
    if stopped.is_some() {
        return Err("The solver run was stopped.".into());
    }
    mapper.result()
}

/// Turns a solver feed into a delegation's child chat events and keeps
/// what its result needs.
#[derive(Default)]
pub struct Mapper {
    entries: Vec<Option<Entry>>,
    wrote_text: bool,
    summary: Option<Value>,
    notice: Option<String>,
}

impl Mapper {
    /// Applies one change of the run.
    pub fn apply(&mut self, event: Event, emit: &mut dyn FnMut(RuntimeEvent)) {
        match event {
            Event::Set { index, entry } => {
                if self.entries.len() <= index {
                    self.entries.resize(index + 1, None);
                }
                let previous = self.entries[index].replace(entry.clone());
                match entry {
                    // The child chat opens with the delegation's own task.
                    Entry::User(_) | Entry::Delegation { .. } => {}
                    Entry::Assistant { text, model, .. } => {
                        let before = match previous {
                            Some(Entry::Assistant { text, .. }) => text,
                            _ => String::new(),
                        };
                        let delta = text.strip_prefix(before.as_str()).unwrap_or(&text);
                        if delta.trim().is_empty() {
                            return;
                        }
                        if let Some(model) = model {
                            emit(RuntimeEvent::Model(model));
                        }
                        let gap = if before.is_empty() && self.wrote_text {
                            "\n\n"
                        } else {
                            ""
                        };
                        self.wrote_text = true;
                        emit(RuntimeEvent::Text(format!("{gap}{delta}")));
                    }
                    Entry::Tool {
                        name,
                        input,
                        output,
                        running,
                    } => {
                        if name == issue_run::SUMMARY && !running {
                            self.summary = Some(input.clone());
                        }
                        // A finished call finds its running entry by name
                        // and input; a changed input matches the newest.
                        let key = match previous {
                            Some(Entry::Tool {
                                input: before,
                                running: true,
                                ..
                            }) if before != input => Value::Null,
                            _ => input,
                        };
                        self.wrote_text = false;
                        emit(RuntimeEvent::Tool {
                            name,
                            input: key,
                            output,
                            running,
                        });
                    }
                }
            }
            Event::Busy(_) => {}
            Event::Notice(text) => {
                let gap = if self.wrote_text { "\n\n" } else { "" };
                self.wrote_text = true;
                emit(RuntimeEvent::Text(format!("{gap}{text}")));
                self.notice = Some(text);
            }
        }
    }

    /// The tool's result: outcome, pull request, cost, and run folder.
    ///
    /// # Errors
    /// The run ended with neither a summary nor its closing line.
    pub fn result(&self) -> Result<Value, String> {
        let notice = self
            .notice
            .as_deref()
            .filter(|text| text.starts_with("Solver ended:"));
        let summary = self.summary.as_ref();
        let outcome = summary
            .and_then(|s| s.pointer("/outcome/status"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                notice?
                    .strip_prefix("Solver ended:")?
                    .split(['.', ' ', '('])
                    .find(|word| !word.is_empty())
                    .map(str::to_owned)
            })
            .ok_or("The solver run ended without a result.")?;
        let reason = summary
            .and_then(|s| s.pointer("/outcome/reason"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let pull_request = summary
            .and_then(|s| s.pointer("/solver/pull_request"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| after(notice?, "Pull request: "));
        let cost = summary.and_then(|s| {
            s.pointer("/solver/cost_usd")
                .and_then(Value::as_f64)
                .or_else(|| s.pointer("/cost/total_usd").and_then(Value::as_f64))
        });
        let folder = summary
            .and_then(|s| s["folder"].as_str())
            .map(str::to_owned)
            .or_else(|| {
                let recording = after(notice?, "Recording: ")?;
                Some(Path::new(&recording).parent()?.display().to_string())
            });
        let issue = summary.and_then(|s| s["issue"].as_u64());
        let title = summary.and_then(|s| s["title"].as_str()).map(str::to_owned);
        let mut reply = match issue {
            Some(issue) => format!("Solver {} on issue #{issue}", words(&outcome)),
            None => format!("Solver {}", words(&outcome)),
        };
        if let Some(reason) = &reason {
            reply.push_str(&format!(" ({})", reason.trim_end_matches('.')));
        }
        reply.push('.');
        match &pull_request {
            Some(url) => reply.push_str(&format!(" Pull request: {url}.")),
            None => reply.push_str(" No pull request was opened."),
        }
        if let Some(cost) = cost {
            reply.push_str(&format!(" Cost: ${cost:.2}."));
        }
        if let Some(folder) = &folder {
            reply.push_str(&format!(" Run folder: {folder}."));
        }
        Ok(json!({
            "outcome": outcome,
            "reason": reason,
            "issue": issue,
            "title": title,
            "pull_request": pull_request,
            "cost_usd": cost,
            "run_folder": folder,
            "reply": reply,
        }))
    }
}

/// An outcome as a phrase: `passed`, `failed`, `stopped during setup`.
fn words(outcome: &str) -> String {
    match outcome {
        "passed" => "passed".into(),
        "failed" => "failed".into(),
        "cancelled" => "was stopped".into(),
        "setup_failed" => "could not set up".into(),
        other => format!("ended: {}", other.replace('_', " ")),
    }
}

/// The word after `label` in `text`, without its closing period.
fn after(text: &str, label: &str) -> Option<String> {
    let rest = &text[text.find(label)? + label.len()..];
    let word = rest.split_whitespace().next()?;
    Some(word.trim_end_matches('.').to_owned()).filter(|word| !word.is_empty())
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// The in-process tool server a claude turn gets while the plugin is
/// on: one `solver` tool whose progress reaches `events` as a delegation,
/// stopped when `stop` is set.
#[must_use]
pub fn mcp_server(
    cwd: PathBuf,
    events: tokio::sync::mpsc::UnboundedSender<RuntimeEvent>,
    stop: Arc<AtomicBool>,
) -> claude_agent_sdk::SdkMcpServer {
    claude_agent_sdk::SdkMcpServer::new(MCP_SERVER, "1.0.0")
        // A run takes many minutes; the tool call must not time out first.
        .timeout_ms(6 * 60 * 60 * 1000)
        .tool(TOOL, DESCRIPTION, schema(), move |input: Value| {
            let cwd = cwd.clone();
            let events = events.clone();
            let stop = stop.clone();
            async move {
                let id = format!(
                    "solver-{}-{}",
                    std::process::id(),
                    NEXT_ID.fetch_add(1, Ordering::Relaxed)
                );
                let task = task(&input).unwrap_or_else(|| "Solve an issue.".into());
                let mut emit = |event: RuntimeEvent| {
                    let _ = events.send(RuntimeEvent::Delegation {
                        id: id.clone(),
                        name: NAME.into(),
                        task: task.clone(),
                        event: Box::new(event),
                    });
                };
                emit(RuntimeEvent::Tool {
                    name: TOOL.into(),
                    input: input.clone(),
                    output: Value::Null,
                    running: true,
                });
                // Claude Code calls this, so its plugin is on.
                let result = execute_in(&input, &cwd, true, None, &stop, &mut emit).await;
                let output = match &result {
                    Ok(value) => value.clone(),
                    Err(error) => json!({"error": error}),
                };
                emit(RuntimeEvent::Tool {
                    name: TOOL.into(),
                    input: Value::Null,
                    output: output.clone(),
                    running: false,
                });
                Ok(match result {
                    Ok(value) => claude_agent_sdk::ToolResult::text(value.to_string()),
                    Err(error) => claude_agent_sdk::ToolResult::error(error),
                })
            }
        })
}

/// The tool server's name: its tool is `mcp__coder__solver` to the engine.
/// The same server name the floors strip ([`coder_sdk::floors::SERVER`]).
pub const MCP_SERVER: &str = coder_sdk::floors::SERVER;

/// Whether `name` is the solver tool as a claude turn's engine names it.
#[must_use]
pub fn is_mcp_tool(name: &str) -> bool {
    name == format!("mcp__{MCP_SERVER}__{TOOL}")
}

#[cfg(test)]
mod tests;
