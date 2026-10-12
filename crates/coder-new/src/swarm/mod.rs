//! The solver swarm (#11346): the live model hands several issues to
//! several solvers at once, one per issue, each in its own worktree and run
//! folder as solver runs are today. They coordinate over the agent channel
//! ([`crate::comms`]); with the channel off they run side by side without
//! talking.
//!
//! `docs/coder/subagent-channel.md` is the spec. Concurrency defaults to
//! what the machine fits, and a spend limit, when the person sets one,
//! stops new solvers from starting once the finished ones have spent it.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use serde_json::{Value, json};

use crate::comms::{Channel, Intent};
use crate::solver;

/// The plugin's ID in /plugins and in `bundled-plugins.json`.
pub const PLUGIN: &str = "solver-swarm";

/// The tool the live model calls.
pub const TOOL: &str = "solver_swarm";

/// The delegation's name where a person reads it.
pub const NAME: &str = "Solver swarm";

/// How many solvers run at once when the person sets none: a quarter of
/// the cores, at least one and at most four.
#[must_use]
pub fn default_concurrency() -> usize {
    let cores = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    (cores / 4).clamp(1, 4)
}

/// What the model is told about the tool.
#[must_use]
pub const fn instructions() -> &'static str {
    "Solver swarm hands several GitHub issues to several Solvers at once, one per issue, each in its own worktree and run folder. They post what they mean to change to the agent channel and settle overlaps with each other. Use it when the person asks to solve several issues at once, or a whole list or board column. `issues` is a list of N, #N, or owner/repo#N. Set `no_pr` to stop each run after its checks; `concurrency` and `budget_usd` are limits the person may set. It returns one row per issue: outcome, pull request, cost, and the folder with the run's files.\n"
}

/// The tool's input schema.
#[must_use]
pub fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "issues": {
                "type": "array",
                "items": {"type": ["string", "integer"]},
                "description": "The issues to solve, each N, #N, or owner/repo#N."
            },
            "no_pr": {"type": "boolean", "description": "Stop after each run's checks; open no pull requests."},
            "concurrency": {"type": "integer", "description": "How many solvers run at once."},
            "budget_usd": {"type": "number", "description": "Stop starting solvers once the finished ones have spent this."}
        },
        "required": ["issues"],
        "additionalProperties": false
    })
}

/// The tool's definition, in the chat-completions function shape.
#[must_use]
pub fn tool_definition() -> Value {
    json!({"type":"function","function":{
        "name": TOOL,
        "description": "Hand several GitHub issues to several Solvers at once, one per issue; they coordinate over the agent channel. Each returns its outcome, pull request, cost, and run folder.",
        "parameters": schema(),
    }})
}

/// What a call asks for: the issues and the person's limits.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub issues: Vec<(Option<String>, u64)>,
    pub no_pr: bool,
    pub concurrency: usize,
    pub budget_usd: Option<f64>,
}

/// Reads a call's arguments.
///
/// # Errors
/// The arguments are not `{issues, no_pr?, concurrency?, budget_usd?}`, an
/// issue is not N, #N, or owner/repo#N, or a limit is out of range.
pub fn arguments(arguments: &Value) -> Result<Request, String> {
    const USAGE: &str = "solver_swarm takes issues (a list of N, #N, or owner/repo#N) and optionally no_pr, concurrency, and budget_usd, with no other fields.";
    let object = arguments.as_object().ok_or(USAGE)?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "issues" | "no_pr" | "concurrency" | "budget_usd"
        )
    }) {
        return Err(USAGE.into());
    }
    let no_pr = match object.get("no_pr") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err(USAGE.into()),
    };
    let concurrency = match object.get("concurrency") {
        None | Some(Value::Null) => default_concurrency(),
        Some(Value::Number(number)) => number
            .as_u64()
            .filter(|n| (1..=32).contains(n))
            .map(|n| n as usize)
            .ok_or("concurrency takes a number from 1 to 32.")?,
        Some(_) => return Err(USAGE.into()),
    };
    let budget_usd = match object.get("budget_usd") {
        None | Some(Value::Null) => None,
        Some(Value::Number(number)) => Some(
            number
                .as_f64()
                .filter(|value| *value > 0.0)
                .ok_or("budget_usd takes a number above zero.")?,
        ),
        Some(_) => return Err(USAGE.into()),
    };
    let list = object
        .get("issues")
        .and_then(Value::as_array)
        .ok_or(USAGE)?;
    if list.is_empty() {
        return Err("Name at least one issue to solve.".into());
    }
    if list.len() > 64 {
        return Err("A swarm solves at most 64 issues at once.".into());
    }
    let mut issues = Vec::new();
    for item in list {
        let text = match item {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            _ => return Err(USAGE.into()),
        };
        let reference = solver::issue_reference(&text).ok_or(USAGE)?;
        if !issues.contains(&reference) {
            issues.push(reference);
        }
    }
    Ok(Request {
        issues,
        no_pr,
        concurrency,
        budget_usd,
    })
}

/// The delegation's task line: what the swarm's child chat opens with.
#[must_use]
pub fn task(request: &Request) -> String {
    let list: Vec<String> = request
        .issues
        .iter()
        .map(|(repo, issue)| match repo {
            Some(repo) => format!("{repo}#{issue}"),
            None => format!("#{issue}"),
        })
        .collect();
    let tail = if request.no_pr {
        ", without opening pull requests."
    } else {
        "."
    };
    format!(
        "Solve {} issue{} at once{tail}",
        list.len(),
        if list.len() == 1 { "" } else { "s" },
    ) + &format!(" Issues: {}.", list.join(", "))
}

/// A solver's options for one issue in a swarm, at `cwd`.
///
/// # Errors
/// The run options cannot be built.
pub fn options(
    request: &Request,
    issue: &(Option<String>, u64),
    cwd: &std::path::Path,
) -> Result<solver::Options, String> {
    let mut args = vec![
        issue.1.to_string(),
        "--repo".into(),
        cwd.display().to_string(),
    ];
    if let Some(repo) = &issue.0 {
        args.extend(["--github".into(), repo.clone()]);
    }
    if request.no_pr {
        args.push("--no-pr".into());
    }
    solver::Options::parse(&args)
}

/// One solver the swarm started.
#[derive(Clone, Debug)]
pub struct Member {
    pub name: String,
    pub issue: u64,
    pub repo: Option<String>,
}

impl Member {
    /// The member's name on the channel.
    #[must_use]
    pub fn channel_name(issue: u64) -> String {
        format!("solver-{issue}")
    }
}

/// The intents a finished solver leaves: the files its Coder actually
/// changed, and the crates and resources those touch. Bounded to the
/// files the run's summary names.
#[must_use]
pub fn intent_from_summary(member: &str, summary: &Value) -> Option<Intent> {
    let files: Vec<String> = summary
        .get("changed")
        .and_then(Value::as_array)
        .or_else(|| summary.pointer("/diff/files").and_then(Value::as_array))
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if files.is_empty() {
        return None;
    }
    Some(Intent::new(member, files, Vec::new()))
}

/// The state root a swarm's solvers live under; `None` for the default.
#[must_use]
pub fn solver_state(options: &solver::Options) -> PathBuf {
    options.inner.state.clone()
}

/// Runs a swarm until every solver ends or `cancel` is set. Emits one
/// `RuntimeEvent`-shaped change per solver as a delegation.
///
/// This is the in-process driver the tool and the CLI share: it starts one
/// solver per issue, bounded by `concurrency`, and joins the agent channel
/// when one is given.
pub struct Run {
    pub request: Request,
    pub cwd: PathBuf,
    pub channel: Option<Channel>,
    pub cancel: Arc<AtomicBool>,
}

/// The stage a solver reached, for the swarm view's row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Queued,
    Running,
    Done,
}

impl Stage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Done => "done",
        }
    }
}

/// One solver's row in the swarm view.
#[derive(Clone, Debug)]
pub struct Row {
    pub member: String,
    pub issue: u64,
    pub stage: Stage,
    pub outcome: Option<String>,
    pub pull_request: Option<String>,
    pub cost_usd: Option<f64>,
    pub folder: Option<PathBuf>,
}

impl Row {
    fn json(&self) -> Value {
        json!({
            "member": self.member,
            "issue": self.issue,
            "stage": self.stage.as_str(),
            "outcome": self.outcome,
            "pull_request": self.pull_request,
            "cost_usd": self.cost_usd,
            "folder": self.folder.as_ref().map(|folder| folder.display().to_string()),
        })
    }
}

/// The swarm's summary: one row per solver, the agreed merge order, the
/// overlaps settled, failures with reasons, and the total cost.
#[must_use]
pub fn summary(request: &Request, rows: &[Row], agreements: &[crate::comms::Agreement]) -> Value {
    let total: Option<f64> = {
        let known: Vec<f64> = rows.iter().filter_map(|row| row.cost_usd).collect();
        (!known.is_empty()).then(|| known.iter().sum())
    };
    let merge_order: Vec<String> = agreements
        .iter()
        .filter(|agreement| agreement.outcome == crate::comms::Outcome::First)
        .map(|agreement| agreement.detail.clone())
        .filter(|name| !name.is_empty())
        .collect();
    let failures: Vec<Value> = rows
        .iter()
        .filter(|row| {
            row.outcome
                .as_deref()
                .is_some_and(|outcome| outcome != "passed")
        })
        .map(|row| json!({"member": row.member, "outcome": row.outcome, "reason": Value::Null}))
        .collect();
    json!({
        "v": "openagents.coder-solver-swarm.v1",
        "request": {
            "issues": request.issues.iter().map(|(repo, issue)| match repo {
                Some(repo) => format!("{repo}#{issue}"),
                None => format!("#{issue}"),
            }).collect::<Vec<_>>(),
            "no_pr": request.no_pr,
            "concurrency": request.concurrency,
            "budget_usd": request.budget_usd,
        },
        "rows": rows.iter().map(Row::json).collect::<Vec<_>>(),
        "merge_order": merge_order,
        "overlaps_settled": agreements.len(),
        "failures": failures,
        "cost": {"total_usd": total, "complete": total.is_some()},
    })
}

/// The swarm's result as the tool's reply, and its rows.
#[must_use]
pub fn reply(rows: &[Row], cost: Option<f64>) -> String {
    let passed = rows
        .iter()
        .filter(|row| row.outcome.as_deref() == Some("passed"))
        .count();
    let mut reply = format!("Solver swarm finished: {passed} of {} passed.", rows.len());
    let prs: Vec<String> = rows
        .iter()
        .filter_map(|row| row.pull_request.clone())
        .collect();
    if !prs.is_empty() {
        reply.push_str(&format!(" Pull requests: {}.", prs.join(", ")));
    }
    if let Some(cost) = cost {
        reply.push_str(&format!(" Cost: ${cost:.2}."));
    }
    reply
}

/// Folds each solver's summary into its row and posts its intent. Both the
/// solver tool's own result (flat `outcome`, `pull_request`, `cost_usd`,
/// `run_folder`) and the run's `summary.json` (nested) are accepted.
pub fn record(summary: &Value, row: &mut Row, channel: Option<&Channel>) {
    row.outcome = summary
        .pointer("/outcome/status")
        .and_then(Value::as_str)
        .or_else(|| summary.get("outcome").and_then(Value::as_str))
        .map(str::to_owned);
    row.pull_request = summary
        .pointer("/solver/pull_request")
        .and_then(Value::as_str)
        .or_else(|| summary.get("pull_request").and_then(Value::as_str))
        .map(str::to_owned);
    row.cost_usd = summary
        .pointer("/solver/cost_usd")
        .and_then(Value::as_f64)
        .or_else(|| summary.pointer("/cost/total_usd").and_then(Value::as_f64))
        .or_else(|| summary.get("cost_usd").and_then(Value::as_f64));
    row.folder = summary
        .get("folder")
        .and_then(Value::as_str)
        .or_else(|| summary.get("run_folder").and_then(Value::as_str))
        .map(PathBuf::from);
    if let Some(channel) = channel {
        if let Some(intent) = intent_from_summary(&row.member, summary) {
            let _ = channel.intent(intent);
        }
    }
}

/// Whether the swarm's spent cost has reached the person's budget, so no
/// further solver should start.
#[must_use]
pub fn over_budget(request: &Request, spent: f64) -> bool {
    request.budget_usd.is_some_and(|budget| spent >= budget)
}

/// A solver the swarm has started: its row, its channel member name, the
/// feed the swarm polls, and the mapper that turns its changes into the
/// child chat's events.
struct Started {
    row: Row,
    member: String,
    feed: crate::issue_run::Feed,
    mapper: crate::solver_plugin::Mapper,
}

/// Runs a swarm: one solver per issue, at most `concurrency` at a time,
/// each in its own worktree and run folder. A solver that finishes starts
/// the next queued one, unless the person's budget has been spent or the
/// caller cancelled. Each solver reaches `emit` as its own delegation, and
/// its changes are posted to the channel so overlaps reach the others.
///
/// # Errors
/// The arguments are invalid, or a solver's options cannot be built.
pub async fn execute(
    arguments: &Value,
    cwd: &std::path::Path,
    channel: Option<Channel>,
    cancel: &AtomicBool,
    emit: &mut (dyn FnMut(crate::bundled_runtime::RuntimeEvent) + Send),
) -> Result<Value, String> {
    use crate::bundled_runtime::RuntimeEvent;
    let request = self::arguments(arguments)?;
    if let Some(channel) = &channel {
        for issue in &request.issues {
            let _ = channel.join(&Member::channel_name(issue.1));
        }
    }
    let task_line = task(&request);
    emit(RuntimeEvent::Delegation {
        id: format!("swarm-{}", crate::comms::now_ms()),
        name: NAME.into(),
        task: task_line.clone(),
        event: Box::new(RuntimeEvent::Text(format!(
            "Starting {} solver(s), {} at a time.\n",
            request.issues.len(),
            request.concurrency
        ))),
    });

    let mut rows: Vec<Row> = request
        .issues
        .iter()
        .map(|(_, issue)| Row {
            member: Member::channel_name(*issue),
            issue: *issue,
            stage: Stage::Queued,
            outcome: None,
            pull_request: None,
            cost_usd: None,
            folder: None,
        })
        .collect();
    let mut active: Vec<Started> = Vec::new();
    let mut next = 0usize;
    let mut spent = 0.0f64;
    let cancelled = || cancel.load(Ordering::Relaxed);
    loop {
        if cancelled() {
            for started in &active {
                started.feed.stop();
            }
        }
        while !cancelled()
            && active.len() < request.concurrency
            && next < request.issues.len()
            && !over_budget(&request, spent)
        {
            let issue = request.issues[next].clone();
            let member = Member::channel_name(issue.1);
            let options = options(&request, &issue, cwd)?;
            emit(RuntimeEvent::Delegation {
                id: member.clone(),
                name: NAME.into(),
                task: format!("Solve issue #{}.", issue.1),
                event: Box::new(RuntimeEvent::Tool {
                    name: TOOL.into(),
                    input: json!({"issue": issue.1}),
                    output: Value::Null,
                    running: true,
                }),
            });
            if let Some(row) = rows.iter_mut().find(|row| row.issue == issue.1) {
                row.stage = Stage::Running;
            }
            active.push(Started {
                row: rows
                    .iter()
                    .find(|row| row.issue == issue.1)
                    .cloned()
                    .expect("every issue has a row"),
                member,
                feed: solver::start(options),
                mapper: crate::solver_plugin::Mapper::default(),
            });
            next += 1;
        }
        if active.is_empty() {
            break;
        }
        let mut finished: Vec<usize> = Vec::new();
        for (index, started) in active.iter_mut().enumerate() {
            let (events, ended) = started.feed.poll();
            for event in events {
                let member = started.member.clone();
                let issue = started.row.issue;
                let mut child = |event: RuntimeEvent| {
                    emit(RuntimeEvent::Delegation {
                        id: member.clone(),
                        name: NAME.into(),
                        task: format!("Solve issue #{issue}."),
                        event: Box::new(event),
                    });
                };
                started.mapper.apply(event, &mut child);
            }
            if ended {
                finished.push(index);
            }
        }
        for index in finished.into_iter().rev() {
            let started = active.remove(index);
            let result = started.mapper.result().unwrap_or(Value::Null);
            let row = rows
                .iter_mut()
                .find(|row| row.issue == started.row.issue)
                .expect("every issue has a row");
            row.outcome = result["outcome"].as_str().map(str::to_owned);
            row.pull_request = result["pull_request"].as_str().map(str::to_owned);
            row.cost_usd = result["cost_usd"].as_f64();
            row.folder = result["run_folder"].as_str().map(PathBuf::from);
            row.stage = Stage::Done;
            spent += row.cost_usd.unwrap_or(0.0);
            emit(RuntimeEvent::Delegation {
                id: started.member.clone(),
                name: NAME.into(),
                task: format!("Solve issue #{}.", started.row.issue),
                event: Box::new(RuntimeEvent::Tool {
                    name: TOOL.into(),
                    input: Value::Null,
                    output: result,
                    running: false,
                }),
            });
            if let Some(channel) = &channel {
                // The solver's own summary names the files it changed, so
                // the others see what it touched before they edit.
                if let Some(folder) = &row.folder {
                    if let Ok(bytes) = std::fs::read(folder.join("summary.json")) {
                        if let Ok(solver_summary) = serde_json::from_slice::<Value>(&bytes) {
                            if let Some(intent) =
                                intent_from_summary(&started.member, &solver_summary)
                            {
                                let _ = channel.intent(intent);
                            }
                        }
                    }
                }
                let _ = channel.finish(&started.member);
            }
        }
        if active.is_empty() && next >= request.issues.len() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    if cancelled() {
        for row in rows.iter_mut().filter(|row| row.stage == Stage::Queued) {
            row.outcome = Some("cancelled".into());
            row.stage = Stage::Done;
        }
    }
    let agreements = match &channel {
        Some(channel) => channel.agreements().unwrap_or_default(),
        None => Vec::new(),
    };
    let summary = summary(&request, &rows, &agreements);
    let cost = summary["cost"]["total_usd"].as_f64();
    let reply = reply(&rows, cost);
    Ok(json!({
        "summary": summary,
        "rows": summary["rows"],
        "merge_order": summary["merge_order"],
        "cost_usd": cost,
        "reply": reply,
    }))
}

/// The cancel flag a swarm's solvers share.
#[must_use]
pub fn cancel_flag() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

/// Stops a running swarm.
pub fn stop(cancel: &AtomicBool) {
    cancel.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests;
