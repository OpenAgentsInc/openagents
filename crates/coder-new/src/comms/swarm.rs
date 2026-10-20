//! The solver swarm (#11346): the live model hands N issues to N solvers
//! at once, and they coordinate through the session's channel.
//!
//! Each solver is the same run as the Solver plugin's ([`crate::solver::
//! start`]), in its own worktree and run folder, shown as its own child
//! chat. While the channel is on, each one joins it: its intents are posted
//! from its run ([`super::watch`]), an overlap between two of them is
//! settled by the swarm's rule (the earlier intent goes first, the other
//! rebases on its branch; [`super::Channel::settle_by_order`]), and each
//! solver's next Coder hand reads what the channel says about it. The
//! swarm's own chat shows the swarm view (a row per solver with its stage,
//! cost and outcome) and the channel.
//!
//! The person may set how many run at once and a spend limit; no step or
//! time limits are imposed. The plugin ([`PLUGIN`]) is on by default.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{Channel, Kind, Member, watch};
use crate::bundled_runtime::RuntimeEvent;
use crate::issue_run::Feed;
use crate::solver_plugin::Mapper;

/// The plugin's ID in /plugins and in `bundled-plugins.json`.
pub const PLUGIN: &str = "solver-swarm";

/// The tool the model calls.
pub const TOOL: &str = "solver_swarm";

/// The delegation's name where a person reads it.
pub const NAME: &str = "Solver swarm";

/// How long stopped solvers are given to write their summaries.
const STOP_GRACE: Duration = Duration::from_secs(20);

/// Free disk under which one solver runs at a time.
const DISK_FLOOR: u64 = 30_000_000_000;

const DESCRIPTION: &str = "Hand several GitHub issues to Solver at once, one solver per issue, each taking its issue to a pull request in its own copy of the repository. The solvers tell each other what they will change, are told when they overlap, and agree who goes first. Use it when the person asks to solve, fix or implement two or more issues together. `issues` lists them (N, #N or owner/repo#N; without a repository, OpenAgentsInc/openagents). Set `no_pr` to stop after the checks without opening pull requests. `concurrency` (how many run at once) and `budget_usd` (stop starting new solvers once this much is spent) are only for when the person sets them. It returns each solver's outcome, pull request and cost, the agreed merge order, and the total cost.";

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
            "issues":{"type":"array","items":{"type":["string","integer"]},"minItems":1,"description":"The issues: N, #N, or owner/repo#N."},
            "no_pr":{"type":"boolean","description":"Stop after the checks; open no pull requests."},
            "concurrency":{"type":"integer","minimum":1,"description":"How many solvers run at once, when the person says."},
            "budget_usd":{"type":"number","exclusiveMinimum":0,"description":"A spend limit the person set, in dollars."}
        },
        "required":["issues"],
        "additionalProperties":false
    })
}

/// What the model is told about the tool.
#[must_use]
pub const fn instructions() -> &'static str {
    "solver_swarm runs one solver per issue at the same time; the solvers coordinate among themselves. When the person asks to solve, fix or implement several issues, call it once with all of them rather than calling solver repeatedly; pass no_pr true when they ask for no pull requests, and concurrency or budget_usd only when they set them. Report each issue's outcome and pull request, the merge order, and the total cost briefly.\n"
}

/// A call's arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct Arguments {
    pub issues: Vec<(Option<String>, u64)>,
    pub no_pr: bool,
    pub concurrency: Option<usize>,
    pub budget_usd: Option<f64>,
}

/// Reads a call.
///
/// # Errors
/// The fields are not the schema's, or an issue is not N, #N or
/// owner/repo#N.
pub fn arguments(value: &Value) -> Result<Arguments, String> {
    const USAGE: &str = "solver_swarm takes issues (a list of N, #N or owner/repo#N) and optionally no_pr, concurrency and budget_usd.";
    let object = value.as_object().ok_or(USAGE)?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "issues" | "no_pr" | "concurrency" | "budget_usd"
        )
    }) {
        return Err(USAGE.into());
    }
    let mut issues = Vec::new();
    for item in object
        .get("issues")
        .and_then(Value::as_array)
        .ok_or(USAGE)?
    {
        let text = match item {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            _ => return Err(USAGE.into()),
        };
        let issue = crate::solver::issue_reference(&text).ok_or(USAGE)?;
        if !issues.contains(&issue) {
            issues.push(issue);
        }
    }
    if issues.is_empty() {
        return Err(USAGE.into());
    }
    let no_pr = match object.get("no_pr") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err(USAGE.into()),
    };
    let concurrency = match object.get("concurrency") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .filter(|n| *n >= 1)
                .and_then(|n| usize::try_from(n).ok())
                .ok_or(USAGE)?,
        ),
    };
    let budget_usd = match object.get("budget_usd") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_f64().filter(|n| *n > 0.0).ok_or(USAGE)?),
    };
    Ok(Arguments {
        issues,
        no_pr,
        concurrency,
        budget_usd,
    })
}

fn reference((repo, issue): &(Option<String>, u64)) -> String {
    match repo {
        Some(repo) => format!("{repo}#{issue}"),
        None => format!("#{issue}"),
    }
}

/// The delegation's task line.
#[must_use]
pub fn task(value: &Value) -> Option<String> {
    let args = arguments(value).ok()?;
    let names: Vec<String> = args.issues.iter().map(reference).collect();
    let list = match names.as_slice() {
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        [] => return None,
    };
    Some(if args.no_pr {
        format!("Solve {list} together, without opening pull requests.")
    } else {
        format!("Solve {list} together, each to a pull request.")
    })
}

/// How many solvers run at once by default: a quarter of the cores, one to
/// four, and one when the disk is nearly full; never more than `issues`.
#[must_use]
pub fn default_concurrency(cwd: &Path, issues: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let mut fits = (cores / 4).clamp(1, 4);
    if coder_lease::free_disk(cwd).is_ok_and(|free| free < DISK_FLOOR) {
        fits = 1;
    }
    fits.min(issues).max(1)
}

/// One solver to run.
#[derive(Clone, Debug)]
pub struct Plan {
    pub issue: u64,
    pub options: crate::solver::Options,
    /// The `solver` call it stands for, shown as its child chat's tool.
    pub arguments: Value,
}

/// What the person set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub concurrency: usize,
    pub budget_usd: Option<f64>,
}

/// Runs a call made in `cwd`.
///
/// # Errors
/// The arguments are invalid, or every solver failed to start.
pub async fn execute(
    value: &Value,
    cwd: &Path,
    channel: Option<&Channel>,
    cancel: &AtomicBool,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<Value, String> {
    let args = arguments(value)?;
    let mut plans = Vec::new();
    for issue in &args.issues {
        let arguments = json!({"issue": reference(issue), "no_pr": args.no_pr});
        let options = crate::solver_plugin::options(&arguments, cwd)?;
        plans.push(Plan {
            issue: issue.1,
            options,
            arguments,
        });
    }
    let limits = Limits {
        concurrency: args
            .concurrency
            .unwrap_or_else(|| default_concurrency(cwd, plans.len())),
        budget_usd: args.budget_usd,
    };
    run(plans, limits, channel, cancel, emit).await
}

/// An outcome in plain words.
#[must_use]
pub fn outcome_words(outcome: &str) -> String {
    match outcome {
        "passed" => "passed".into(),
        "failed" => "failed".into(),
        "cancelled" | "stopped" => "was stopped".into(),
        "setup_failed" => "could not set up".into(),
        "not_started" => "not started".into(),
        other => other.replace('_', " "),
    }
}

/// One solver's place in the swarm.
struct Slot {
    plan: Plan,
    name: String,
    id: String,
    task: String,
    stage: &'static str,
    cost: Option<f64>,
    outcome: Option<String>,
    reason: Option<String>,
    pull_request: Option<String>,
    run: Option<Running>,
    /// Its place in the channel; it keeps reading after it ends, so a later
    /// overlap with its branch still reaches its chat.
    member: Option<Member>,
    cursor: usize,
}

struct Running {
    feed: Feed,
    mapper: Mapper,
    watch: watch::SolverWatch,
}

impl Slot {
    fn row(&self) -> Value {
        json!({
            "name": self.name,
            "issue": self.plan.issue,
            "stage": self.stage,
            "cost_usd": self.cost,
            "outcome": self.outcome,
            "reason": self.reason,
            "pull_request": self.pull_request,
        })
    }

    fn nested(&self, event: RuntimeEvent) -> RuntimeEvent {
        RuntimeEvent::Delegation {
            id: self.id.clone(),
            name: self.name.clone(),
            task: self.task.clone(),
            event: Box::new(event),
        }
    }
}

static NEXT_SWARM: AtomicU64 = AtomicU64::new(1);

/// Runs `plans` within `limits`, coordinating through `channel` when it is
/// on, until every solver ends or `cancel` is set.
///
/// # Errors
/// No plans.
#[allow(clippy::too_many_lines)]
pub async fn run(
    plans: Vec<Plan>,
    limits: Limits,
    channel: Option<&Channel>,
    cancel: &AtomicBool,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<Value, String> {
    if plans.is_empty() {
        return Err("A swarm needs at least one issue.".into());
    }
    let swarm = format!(
        "swarm-{}-{}",
        std::process::id(),
        NEXT_SWARM.fetch_add(1, Ordering::Relaxed)
    );
    let mut slots: Vec<Slot> = plans
        .into_iter()
        .map(|plan| {
            let task = crate::solver_plugin::task(&plan.arguments)
                .unwrap_or_else(|| format!("Solve issue #{}.", plan.issue));
            Slot {
                name: format!("Solver #{}", plan.issue),
                id: format!("{swarm}-{}", plan.issue),
                task,
                plan,
                stage: "Waiting",
                cost: None,
                outcome: None,
                reason: None,
                pull_request: None,
                run: None,
                member: None,
                cursor: 0,
            }
        })
        .collect();
    let concurrency = limits.concurrency.max(1);
    let mut cursor = channel.map_or(0, |channel| channel.records(0).1);
    let mut spent = 0.0_f64;
    let mut stopping: Option<Instant> = None;
    let view = |slots: &[Slot], spent: f64, summary: Option<&str>| {
        json!({
            "swarm": swarm,
            "members": slots.iter().map(Slot::row).collect::<Vec<_>>(),
            "spent_usd": spent,
            "concurrency": concurrency,
            "budget_usd": limits.budget_usd,
            "summary": summary,
        })
    };
    let swarm_input = json!({"swarm": swarm});
    emit(RuntimeEvent::Tool {
        name: super::view::SWARM.into(),
        input: swarm_input.clone(),
        output: view(&slots, spent, None),
        running: true,
    });
    loop {
        let mut dirty = false;
        // Start solvers while there is room and money.
        if stopping.is_none() {
            loop {
                let running = slots.iter().filter(|slot| slot.run.is_some()).count();
                if running >= concurrency {
                    break;
                }
                let Some(index) = slots
                    .iter()
                    .position(|slot| slot.run.is_none() && slot.outcome.is_none())
                else {
                    break;
                };
                let slot = &mut slots[index];
                dirty = true;
                if limits.budget_usd.is_some_and(|budget| spent >= budget) {
                    slot.outcome = Some("not_started".into());
                    slot.stage = "Not started";
                    slot.reason = Some("The spend limit was reached.".into());
                    continue;
                }
                let member = channel.and_then(|channel| channel.join("solver", &slot.name).ok());
                if let Some(member) = &member {
                    slot.name = member.name().to_owned();
                }
                let mut options = slot.plan.options.clone();
                options.channel = member.clone();
                let feed = crate::solver::start(options);
                slot.stage = "Starting";
                emit(slot.nested(RuntimeEvent::Tool {
                    name: crate::solver_plugin::TOOL.into(),
                    input: slot.plan.arguments.clone(),
                    output: Value::Null,
                    running: true,
                }));
                slot.cursor = channel.map_or(0, |channel| channel.records(0).1);
                slot.member = member;
                slot.run = Some(Running {
                    feed,
                    mapper: Mapper::default(),
                    watch: watch::SolverWatch::default(),
                });
            }
        }
        // Follow each running solver.
        for index in 0..slots.len() {
            let Some(mut running) = slots[index].run.take() else {
                continue;
            };
            let (events, ended) = running.feed.poll();
            for event in events {
                for change in running.watch.see(&event) {
                    match change {
                        watch::Change::Stage(stage) => {
                            slots[index].stage = stage;
                            dirty = true;
                        }
                        watch::Change::Intent { files, note } => {
                            if let (Some(member), Some(channel)) = (&slots[index].member, channel) {
                                if let Ok(overlaps) = member.intent(&files, &[], &[], note) {
                                    for overlap in overlaps {
                                        let _ = channel.settle_by_order(&overlap, NAME);
                                    }
                                }
                            }
                        }
                    }
                }
                let slot = &slots[index];
                running
                    .mapper
                    .apply(event, &mut |event| emit(slot.nested(event)));
            }
            if !ended {
                slots[index].run = Some(running);
                continue;
            }
            dirty = true;
            let result = running.mapper.result();
            let slot = &mut slots[index];
            let output = match &result {
                Ok(value) => {
                    slot.outcome = value["outcome"].as_str().map(str::to_owned);
                    slot.reason = value["reason"].as_str().map(str::to_owned);
                    slot.pull_request = value["pull_request"].as_str().map(str::to_owned);
                    slot.cost = value["cost_usd"].as_f64();
                    value.clone()
                }
                Err(error) => {
                    slot.outcome = Some(
                        if stopping.is_some() {
                            "stopped"
                        } else {
                            "failed"
                        }
                        .into(),
                    );
                    slot.reason = Some(error.clone());
                    json!({"error": error})
                }
            };
            spent += slot.cost.unwrap_or(0.0);
            if slot.stage != "Done" {
                slot.stage = "Ended";
            }
            if let Some(member) = &slot.member {
                let _ = member.finish(&outcome_words(slot.outcome.as_deref().unwrap_or("failed")));
            }
            emit(slot.nested(RuntimeEvent::Tool {
                name: crate::solver_plugin::TOOL.into(),
                input: Value::Null,
                output,
                running: false,
            }));
        }
        // Each solver's chat shows what the channel says to it, before and
        // after it ends.
        for slot in &mut slots {
            if let Some(member) = &slot.member {
                for record in member.inbox(&mut slot.cursor) {
                    let event = slot.nested(super::view::event(&record));
                    emit(event);
                }
            }
        }
        // The swarm's own chat shows the channel.
        if let Some(channel) = channel {
            let (records, next) = channel.records(cursor);
            cursor = next;
            for record in records.iter().filter(|record| record.kind != Kind::Join) {
                emit(super::view::event(record));
            }
        }
        if dirty {
            emit(RuntimeEvent::Tool {
                name: super::view::SWARM.into(),
                input: swarm_input.clone(),
                output: view(&slots, spent, None),
                running: true,
            });
        }
        let waiting = slots
            .iter()
            .any(|slot| slot.run.is_none() && slot.outcome.is_none());
        let running = slots.iter().any(|slot| slot.run.is_some());
        if !running && (!waiting || stopping.is_some()) {
            break;
        }
        if stopping.is_none() && cancel.load(Ordering::Relaxed) {
            stopping = Some(Instant::now());
            for slot in &slots {
                if let Some(running) = &slot.run {
                    running.feed.stop();
                }
            }
        }
        if stopping.is_some_and(|at| at.elapsed() > STOP_GRACE) {
            for slot in &mut slots {
                if slot.run.take().is_some() {
                    slot.outcome = Some("stopped".into());
                    slot.stage = "Ended";
                    if let Some(member) = &slot.member {
                        let _ = member.finish("was stopped");
                    }
                }
            }
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    for slot in &mut slots {
        if slot.outcome.is_none() {
            slot.outcome = Some(
                if stopping.is_some() {
                    "stopped"
                } else {
                    "not_started"
                }
                .into(),
            );
            slot.stage = "Not started";
        }
    }
    let agreements = channel
        .and_then(|channel| channel.read_state().ok())
        .map(|state| {
            let names: Vec<&str> = slots.iter().map(|slot| slot.name.as_str()).collect();
            state
                .agreements
                .into_iter()
                .filter(|agreement| {
                    agreement
                        .order
                        .iter()
                        .all(|name| names.contains(&name.as_str()))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let summary = summarize(&slots, &agreements, spent);
    emit(RuntimeEvent::Tool {
        name: super::view::SWARM.into(),
        input: swarm_input,
        output: view(&slots, spent, Some(&summary)),
        running: false,
    });
    Ok(json!({
        "members": slots.iter().map(Slot::row).collect::<Vec<_>>(),
        "agreements": agreements,
        "merge_order": merge_order(&slots, &agreements),
        "spent_usd": spent,
        "reply": summary,
    }))
}

/// The order to merge in: each agreed `first` before its follower, the rest
/// as they are.
fn merge_order(slots: &[Slot], agreements: &[super::Agreement]) -> Vec<String> {
    let mut names: Vec<String> = slots
        .iter()
        .filter(|slot| slot.outcome.as_deref() == Some("passed"))
        .map(|slot| slot.name.clone())
        .collect();
    // A few passes settle any chain of "A before B".
    for _ in 0..names.len() {
        for agreement in agreements
            .iter()
            .filter(|a| a.outcome == super::Outcome::First)
        {
            let [first, then, ..] = agreement.order.as_slice() else {
                continue;
            };
            if let (Some(a), Some(b)) = (
                names.iter().position(|name| name == first),
                names.iter().position(|name| name == then),
            ) {
                if a > b {
                    let moved = names.remove(a);
                    names.insert(b, moved);
                }
            }
        }
    }
    names
}

/// The swarm's closing summary, in plain sentences.
fn summarize(slots: &[Slot], agreements: &[super::Agreement], spent: f64) -> String {
    let passed = slots
        .iter()
        .filter(|slot| slot.outcome.as_deref() == Some("passed"))
        .count();
    let mut text = format!("{passed} of {} solvers passed.", slots.len());
    let pulls: Vec<String> = slots
        .iter()
        .filter_map(|slot| Some(format!("{}: {}", slot.name, slot.pull_request.as_ref()?)))
        .collect();
    if pulls.is_empty() {
        text.push_str(" No pull requests were opened.");
    } else {
        text.push_str(&format!(" Pull requests: {}.", pulls.join("; ")));
    }
    if agreements.is_empty() {
        text.push_str(" No overlaps between them.");
    } else {
        text.push_str(&format!(
            " {} overlap{} settled: {}",
            agreements.len(),
            if agreements.len() == 1 { "" } else { "s" },
            agreements
                .iter()
                .map(super::Agreement::sentence)
                .collect::<Vec<_>>()
                .join(" ")
        ));
        let order = merge_order(slots, agreements);
        if order.len() > 1 {
            text.push_str(&format!(" Merge in this order: {}.", order.join(", then ")));
        }
    }
    for slot in slots
        .iter()
        .filter(|slot| slot.outcome.as_deref() != Some("passed"))
    {
        let outcome = outcome_words(slot.outcome.as_deref().unwrap_or("failed"));
        match &slot.reason {
            Some(reason) => text.push_str(&format!(
                " {} {outcome}: {}.",
                slot.name,
                reason.trim_end_matches('.')
            )),
            None => text.push_str(&format!(" {} {outcome}.", slot.name)),
        }
    }
    text.push_str(&format!(" Total cost: ${spent:.2}."));
    text
}
