//! Fan-out for multi-part asks (#11539): the independent parts of a plan
//! run at the same time, each as a delegated child in its own worktree,
//! and one converge step brings their changes back.
//!
//! - **Which part needs which** is System One's judgment, asked in the
//!   same request as `dispatch-v1`'s ([`crate::dispatch`]): one small
//!   question per pair of parts (`fanout-v1`, [`questions`]), "can item j
//!   be done only after item i?", for every earlier item i. List order is
//!   the order work may flow in, so the graph has no cycles. No answer,
//!   or a graph where every part waits for the one before, runs the plan
//!   one part at a time as before.
//! - **Independent parts fan out** through the one `delegate` path
//!   ([`coder_sdk::delegation::Delegator`]): each child gets its own
//!   worktree and branch, under its harness's hooks, floors and the write
//!   guard, and shows on the agent tray as it works. A part that needs
//!   others waits for them and starts from a commit holding their work
//!   ([`coder_sdk::delegation::Child::start`]); when one of them failed it
//!   does not start. At most [`cap`] run at once: the person's
//!   `parallel_parts` in /preferences' file, else [`CAP_ENV`], else a
//!   default from the machine's cores.
//! - **Converge** ([`converge`]): each finished part's own changes are
//!   applied to the chat's checkout in list order, checked first so a part
//!   whose changes don't apply cleanly (another part, or the person,
//!   changed the same lines) is reported with its files and left on its
//!   branch, never forced. Then the checks the parts ran are run once on
//!   the result, and one report lists every part's outcome.
//! - **Records**: one `fanout` line per plan with the parts, the judged
//!   edges and their probabilities, the cap, and each part's outcome.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use coder_sdk::delegation::{Delegator, Outcome as Ended, Report};
use futures::StreamExt;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::bundled_runtime::RuntimeEvent;
use crate::plugin_tools::ExecutionSettings;

/// The question set's frozen name.
pub const QUESTION_SET: &str = "fanout-v1";

/// The variable that sets how many parts run at once, under the person's
/// saved setting.
pub const CAP_ENV: &str = "CODER_PARALLEL_PARTS";

/// The most parts that may run at once, whatever is set.
pub const CAP_MAX: usize = 32;

/// The question id for whether part `part` (from one) needs part `on`.
#[must_use]
pub fn question_id(part: usize, on: usize) -> String {
    format!("part_{part}_needs_{on}")
}

/// One question per pair of an `n`-item list: whether the later item can
/// be done only after the earlier one.
#[must_use]
pub fn questions(n: usize) -> Vec<(String, jev::Noul)> {
    let mut questions = Vec::new();
    for part in 2..=n {
        for on in 1..part {
            questions.push((
                question_id(part, on),
                jev::Noul::new(format!(
                    "Item {part} of the list in the person's latest message: can it be done only after item {on} is done, because it needs item {on}'s changes, files or answer? Answer no when the two could be done at the same time by two people working separately, even in the same project."
                )),
            ));
        }
    }
    questions
}

/// One judged pair: how likely part `part` needs part `on` (both from
/// one, `on` earlier).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Need {
    pub part: u32,
    pub on: u32,
    pub p: f64,
}

/// The judged needs of an `n`-part list from System One's answer; `None`
/// when any pair is missing.
#[must_use]
pub fn needs(response: &jev::SystemOneResponse, n: usize) -> Option<Vec<Need>> {
    let mut needs = Vec::new();
    for part in 2..=n {
        for on in 1..part {
            let answer = response.noul(&question_id(part, on)).ok()?;
            needs.push(Need {
                part: u32::try_from(part).ok()?,
                on: u32::try_from(on).ok()?,
                p: answer.noul,
            });
        }
    }
    Some(needs)
}

/// The parts and which needs which.
#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub parts: Vec<String>,
    /// Every judged pair, with its probability.
    pub needs: Vec<Need>,
}

impl Graph {
    /// The parts (from zero) that part `index` waits for.
    #[must_use]
    pub fn inputs(&self, index: usize) -> Vec<usize> {
        let mut inputs: Vec<usize> = self
            .needs
            .iter()
            .filter(|need| need.p >= 0.5 && need.part as usize == index + 1)
            .map(|need| need.on as usize - 1)
            .collect();
        inputs.sort_unstable();
        inputs.dedup();
        inputs
    }

    /// The edges kept, `(part, on)` from one.
    #[must_use]
    pub fn edges(&self) -> Vec<(u32, u32)> {
        self.needs
            .iter()
            .filter(|need| need.p >= 0.5)
            .map(|need| (need.part, need.on))
            .collect()
    }

    /// Each part's depth: zero for a part that needs nothing, else one
    /// more than its deepest input.
    #[must_use]
    pub fn levels(&self) -> Vec<usize> {
        let mut levels = vec![0; self.parts.len()];
        for index in 0..self.parts.len() {
            levels[index] = self
                .inputs(index)
                .into_iter()
                .map(|input| levels[input] + 1)
                .max()
                .unwrap_or(0);
        }
        levels
    }

    /// Whether two or more parts can run at the same time.
    #[must_use]
    pub fn parallel(&self) -> bool {
        let levels = self.levels();
        (0..self.parts.len()).any(|level| levels.iter().filter(|l| **l == level).count() > 1)
    }

    /// A graph for `plan` from System One's needs, when some of its parts
    /// can run at once; `None` runs the plan one part at a time.
    #[must_use]
    pub fn fan(parts: &[String], needs: Option<Vec<Need>>) -> Option<Self> {
        let graph = Graph {
            parts: parts.to_vec(),
            needs: needs?,
        };
        graph.parallel().then_some(graph)
    }
}

/// How many parts run at once: `saved` (the person's setting), else
/// [`CAP_ENV`], else a quarter of the machine's cores, from two to four.
#[must_use]
pub fn cap(saved: Option<u32>) -> usize {
    let set = saved
        .map(|n| n as usize)
        .or_else(|| std::env::var(CAP_ENV).ok()?.trim().parse().ok())
        .filter(|n| *n > 0);
    set.unwrap_or_else(default_cap).min(CAP_MAX)
}

/// A quarter of the cores, from two to four: each part is an agent with
/// its own tools, builds and tests.
#[must_use]
pub fn default_cap() -> usize {
    std::thread::available_parallelism()
        .map_or(2, std::num::NonZeroUsize::get)
        .div_ceil(4)
        .clamp(2, 4)
}

/// Whether multi-part asks fan out. `CODER_FANOUT=off` keeps plans one
/// part at a time.
#[must_use]
pub fn enabled() -> bool {
    std::env::var("CODER_FANOUT").map_or(true, |value| value.trim() != "off")
}

/// What the chat shows when a fan-out starts.
#[must_use]
pub fn outline(graph: &Graph, cap: usize) -> String {
    let levels = graph.levels();
    let first: Vec<String> = (0..graph.parts.len())
        .filter(|index| levels[*index] == 0)
        .map(|index| (index + 1).to_string())
        .collect();
    let mut text = format!(
        "This is {} separate pieces of work. Parts {} start at the same time, each in its own worktree",
        graph.parts.len(),
        words(&first)
    );
    let waiting: Vec<String> = (0..graph.parts.len())
        .filter(|index| levels[*index] > 0)
        .map(|index| {
            let inputs: Vec<String> = graph
                .inputs(index)
                .into_iter()
                .map(|input| (input + 1).to_string())
                .collect();
            format!("part {} waits for {}", index + 1, words(&inputs))
        })
        .collect();
    if !waiting.is_empty() {
        text.push_str(&format!("; {}", waiting.join(", ")));
    }
    text.push_str(&format!(
        ". Up to {cap} run at once, and their changes come back here when they finish:"
    ));
    for (index, part) in graph.parts.iter().enumerate() {
        text.push_str(&format!(
            "\n{}. {}",
            index + 1,
            part.lines().next().unwrap_or_default()
        ));
    }
    text
}

fn words(numbers: &[String]) -> String {
    match numbers {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// One fan-out, as the worker runs it.
#[derive(Clone)]
pub struct Run {
    pub plan: String,
    /// The text around the list, which every part carries.
    pub around: String,
    pub graph: Graph,
    pub cap: usize,
    /// The harness each part runs on (`delegate`'s `to`), and its model.
    pub to: String,
    pub model: Option<String>,
    pub execution: ExecutionSettings,
    pub record: Option<crate::routing_record::Recorder>,
    pub chat: Option<String>,
}

impl Run {
    /// The task part `index` is handed.
    #[must_use]
    pub fn task(&self, index: usize) -> String {
        let mut text = format!(
            "Part {} of {} of the person's request: {}",
            index + 1,
            self.graph.parts.len(),
            self.graph.parts[index]
        );
        if !self.around.is_empty() {
            text.push_str(&format!("\n\nFrom the whole request: {}", self.around));
        }
        let others: Vec<String> = (0..self.graph.parts.len())
            .filter(|other| *other != index)
            .map(|other| {
                format!(
                    "{}. {}",
                    other + 1,
                    self.graph.parts[other].lines().next().unwrap_or_default()
                )
            })
            .collect();
        if !others.is_empty() {
            text.push_str(&format!(
                "\n\nOther agents do the other parts at the same time; leave them alone:\n{}",
                others.join("\n")
            ));
        }
        text
    }
}

/// How a part ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Its changes are in the chat's checkout.
    Merged,
    /// It finished and changed nothing.
    Done,
    /// Its changes don't apply to the chat's checkout; they stay on its
    /// branch.
    Conflict,
    /// A part it needs was not merged, so its changes stay on its branch.
    Held,
    Failed,
    Stopped,
    /// A part it needs did not finish, so it never started.
    Skipped,
}

/// One part's outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub index: usize,
    pub state: State,
    pub report: Option<Report>,
    /// The commit its worktree started from and the one it ended on.
    pub start: Option<String>,
    pub end: Option<String>,
    /// Files whose changes did not apply.
    pub conflicts: Vec<String>,
    /// Why it did not start or failed, in words.
    pub why: Option<String>,
    pub started_ms: u64,
    pub ended_ms: u64,
}

/// A check run once on the merged result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub command: String,
    pub passed: bool,
}

/// Everything a fan-out did.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub parts: Vec<Part>,
    pub checks: Vec<Checked>,
    /// The most parts that ran at the same time.
    pub widest: usize,
}

/// Runs `run`'s parts through `delegator`, at most [`Run::cap`] at once,
/// each when the parts it needs have finished; `events` carries the
/// children's activity, passed on through `emit`. Then converges.
pub async fn execute(
    run: &Run,
    delegator: Arc<Delegator>,
    mut events: Option<tokio::sync::mpsc::UnboundedReceiver<RuntimeEvent>>,
    cancel: &Arc<AtomicBool>,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Summary {
    let cwd = run.execution.cwd.clone();
    let base = git(&cwd, &["rev-parse", "HEAD"]).ok();
    let n = run.graph.parts.len();
    let mut parts: Vec<Option<Part>> = vec![None; n];
    let mut started = vec![false; n];
    let mut running: futures::stream::FuturesUnordered<BoxFuture<'static, (usize, Part)>> =
        futures::stream::FuturesUnordered::new();
    let mut widest = 0;
    loop {
        let stopping = cancel.load(std::sync::atomic::Ordering::Relaxed);
        // Start what is ready, up to the cap.
        for index in 0..n {
            if started[index] || running.len() >= run.cap.max(1) {
                continue;
            }
            let inputs = run.graph.inputs(index);
            if inputs.iter().any(|input| parts[*input].is_none()) {
                continue;
            }
            started[index] = true;
            if stopping {
                parts[index] = Some(not_started(
                    index,
                    State::Stopped,
                    "Stopped before it started.",
                ));
                continue;
            }
            let unfinished = inputs.iter().copied().find(|input| {
                !parts[*input]
                    .as_ref()
                    .is_some_and(|part| part.state == State::Done)
            });
            if let Some(input) = unfinished {
                parts[index] = Some(not_started(
                    index,
                    State::Skipped,
                    &format!("Part {} did not finish, so it never started.", input + 1),
                ));
                continue;
            }
            let ends: Vec<String> = inputs
                .iter()
                .filter_map(|input| parts[*input].as_ref().and_then(|part| part.end.clone()))
                .collect();
            let start = match combine(&cwd, base.as_deref(), &ends) {
                Ok(start) => start,
                Err(error) => {
                    parts[index] = Some(not_started(index, State::Skipped, &error));
                    continue;
                }
            };
            match start_part(run, &delegator, index, start, emit) {
                Ok(future) => running.push(future),
                Err(part) => parts[index] = Some(*part),
            }
        }
        widest = widest.max(running.len());
        if running.is_empty() {
            if parts.iter().all(Option::is_some) {
                break;
            }
            // Nothing runs and something never became ready: every part
            // left waits for one that never will.
            for (index, part) in parts.iter_mut().enumerate() {
                if part.is_none() {
                    *part = Some(not_started(
                        index,
                        State::Skipped,
                        "Its inputs never finished.",
                    ));
                }
            }
            break;
        }
        let next = async {
            match &mut events {
                Some(events) => loop {
                    tokio::select! {
                        done = running.next() => break done,
                        Some(event) = events.recv() => emit(event),
                    }
                },
                None => running.next().await,
            }
        };
        if let Some((index, part)) = next.await {
            if let Some(report) = &part.report {
                emit(part_event(run, index, report, false));
            }
            parts[index] = Some(part);
        }
    }
    if let Some(events) = &mut events {
        while let Ok(event) = events.try_recv() {
            emit(event);
        }
    }
    let mut parts: Vec<Part> = parts.into_iter().flatten().collect();
    converge(&cwd, &run.graph, &mut parts);
    let checks = if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        Vec::new()
    } else {
        run_checks(&run.execution, &parts, cancel, emit).await
    };
    Summary {
        parts,
        checks,
        widest,
    }
}

fn not_started(index: usize, state: State, why: &str) -> Part {
    let now = atif::now_ms();
    Part {
        index,
        state,
        report: None,
        start: None,
        end: None,
        conflicts: Vec::new(),
        why: Some(why.to_owned()),
        started_ms: now,
        ended_ms: now,
    }
}

/// The delegation event that opens or closes part `index`'s row.
fn part_event(run: &Run, index: usize, report: &Report, running: bool) -> RuntimeEvent {
    RuntimeEvent::Delegation {
        id: report.child.clone(),
        name: name(run, report),
        task: coder_sdk::delegation::clip(run.graph.parts[index].trim(), 200),
        event: Box::new(RuntimeEvent::Tool {
            name: coder_sdk::delegation::TOOL.to_owned(),
            input: json!({"to": report.harness, "part": index + 1}),
            output: if running {
                serde_json::Value::Null
            } else {
                report.to_value()
            },
            running,
        }),
    }
}

fn name(run: &Run, report: &Report) -> String {
    let harness = crate::delegate_host::available(&run.execution, false)
        .into_iter()
        .find(|harness| harness.id == report.harness)
        .map_or_else(|| report.harness.clone(), |harness| harness.name);
    match &report.model {
        Some(model) => format!("{harness} ({model})"),
        None => harness,
    }
}

/// Starts part `index` from `start`, returning the future of its end, or
/// the part refused before it started.
fn start_part(
    run: &Run,
    delegator: &Arc<Delegator>,
    index: usize,
    start: Option<String>,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<BoxFuture<'static, (usize, Part)>, Box<Part>> {
    let mut arguments = json!({"to": run.to, "task": run.task(index)});
    if let Some(model) = &run.model {
        arguments["model"] = json!(model);
    }
    let mut child = delegator
        .prepare(arguments)
        .map_err(|error| Box::new(not_started(index, State::Failed, &error)))?;
    child.start.clone_from(&start);
    let opening = Report::for_child(&child, Ended::Running);
    emit(RuntimeEvent::Delegation {
        id: child.lineage.id.clone(),
        name: crate::delegate_host::label(&child.target),
        task: coder_sdk::delegation::clip(run.graph.parts[index].trim(), 200),
        event: Box::new(RuntimeEvent::Tool {
            name: coder_sdk::delegation::TOOL.to_owned(),
            input: json!({"to": opening.harness, "part": index + 1}),
            output: serde_json::Value::Null,
            running: true,
        }),
    });
    let delegator = delegator.clone();
    Ok(Box::pin(async move {
        let started_ms = atif::now_ms();
        let report = delegator.run_prepared(child).await;
        let end = report.worktree.as_deref().and_then(|worktree| {
            commit_all(worktree, &format!("Part {}", index + 1));
            git(worktree, &["rev-parse", "HEAD"]).ok()
        });
        let state = match report.outcome {
            Ended::Done | Ended::Running => State::Done,
            Ended::Failed => State::Failed,
            Ended::Stopped => State::Stopped,
        };
        let why = report.error.clone();
        (
            index,
            Part {
                index,
                state,
                report: Some(report),
                start,
                end,
                conflicts: Vec::new(),
                why,
                started_ms,
                ended_ms: atif::now_ms(),
            },
        )
    }))
}

/// The commit a part starts from: `base` when it needs nothing, the one
/// part's end when it needs one, else those ends merged into one commit.
///
/// # Errors
/// The parts it needs changed the same lines.
fn combine(cwd: &Path, base: Option<&str>, ends: &[String]) -> Result<Option<String>, String> {
    let Some((first, rest)) = ends.split_first() else {
        return Ok(base.map(str::to_owned));
    };
    let mut at = first.clone();
    for other in rest {
        let tree = git(cwd, &["merge-tree", "--write-tree", &at, other]).map_err(|_| {
            "The parts it needs changed the same lines, so it never started.".to_owned()
        })?;
        let tree = tree.lines().next().unwrap_or_default().to_owned();
        at = git(
            cwd,
            &[
                "-c",
                "user.name=Coder",
                "-c",
                "user.email=coder@openagents.com",
                "commit-tree",
                &tree,
                "-p",
                &at,
                "-p",
                other,
                "-m",
                "The parts this part needs",
            ],
        )?;
    }
    Ok(Some(at))
}

/// Commits whatever the child left in its worktree, so its branch holds
/// all of its work.
fn commit_all(worktree: &Path, message: &str) {
    let _ = git(worktree, &["add", "-A"]);
    let _ = git(
        worktree,
        &[
            "-c",
            "user.name=Coder",
            "-c",
            "user.email=coder@openagents.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "--no-verify",
            "-m",
            message,
        ],
    );
}

/// Brings each finished part's own changes into `cwd`, in list order: a
/// patch that doesn't apply cleanly is reported with its files and left
/// on its part's branch, and a part whose inputs were not all merged is
/// held there too. Worktrees whose changes came in are removed with their
/// branches.
pub fn converge(cwd: &Path, graph: &Graph, parts: &mut [Part]) {
    for index in 0..parts.len() {
        if parts[index].state != State::Done {
            continue;
        }
        let (Some(start), Some(end)) = (parts[index].start.clone(), parts[index].end.clone())
        else {
            continue;
        };
        if start == end {
            remove_checkout(cwd, parts[index].report.as_ref());
            continue;
        }
        // Inputs come earlier in the list, so they are settled by now:
        // merged, or done with nothing to bring.
        let blocked = graph.inputs(index).into_iter().find(|input| {
            parts
                .iter()
                .find(|part| part.index == *input)
                .is_some_and(|part| !matches!(part.state, State::Merged | State::Done))
        });
        if let Some(input) = blocked {
            parts[index].state = State::Held;
            parts[index].why = Some(format!(
                "It builds on part {}, whose changes did not come back.",
                input + 1
            ));
            continue;
        }
        match apply(cwd, &start, &end) {
            Ok(()) => {
                parts[index].state = State::Merged;
                remove_checkout(cwd, parts[index].report.as_ref());
            }
            Err(files) => {
                parts[index].state = State::Conflict;
                parts[index].conflicts = files;
            }
        }
    }
}

/// Applies `start..end` to `cwd`'s working tree when it applies cleanly;
/// else the files that don't.
fn apply(cwd: &Path, start: &str, end: &str) -> Result<(), Vec<String>> {
    let patch = git_bytes(cwd, &["diff", "--binary", start, end]).map_err(|_| Vec::new())?;
    if patch.is_empty() {
        return Ok(());
    }
    let check = git_input(cwd, &["apply", "--check", "--binary", "-"], &patch);
    if let Err(stderr) = check {
        return Err(conflict_files(&stderr));
    }
    git_input(cwd, &["apply", "--binary", "-"], &patch).map_err(|stderr| conflict_files(&stderr))
}

/// The files `git apply` named in its refusal.
fn conflict_files(stderr: &str) -> Vec<String> {
    let mut files: Vec<String> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("error: "))
        .filter_map(|rest| {
            if let Some(path) = rest.strip_prefix("patch failed: ") {
                return path.rsplit_once(':').map(|(path, _)| path.to_owned());
            }
            rest.split_once(": ").map(|(path, _)| path.to_owned())
        })
        .collect();
    files.sort();
    files.dedup();
    files
}

fn remove_checkout(cwd: &Path, report: Option<&Report>) {
    let Some(report) = report else {
        return;
    };
    if let Some(worktree) = &report.worktree {
        let _ = git(
            cwd,
            &["worktree", "remove", "--force", &worktree.to_string_lossy()],
        );
    }
    if let Some(branch) = &report.branch {
        let _ = git(cwd, &["branch", "-D", branch]);
    }
}

/// The checks the merged parts ran, run once on the result in `cwd`,
/// through the same hooks and floors as any command.
async fn run_checks(
    execution: &ExecutionSettings,
    parts: &[Part],
    cancel: &Arc<AtomicBool>,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Vec<Checked> {
    let mut commands: Vec<String> = Vec::new();
    for part in parts.iter().filter(|part| part.state == State::Merged) {
        for check in part.report.iter().flat_map(|report| &report.checks) {
            if !check.command.trim().is_empty() && !commands.contains(&check.command) {
                commands.push(check.command.clone());
            }
        }
    }
    let mut checked = Vec::new();
    for command in commands {
        let result = crate::bundled_runtime::run_command(
            &command,
            &execution.cwd,
            &execution.redaction_keys,
            cancel,
            emit,
        )
        .await;
        let passed = result
            .as_ref()
            .is_ok_and(|output| output["exit"].as_i64() == Some(0));
        emit(RuntimeEvent::Tool {
            name: "Run".into(),
            input: json!({"command": command}),
            output: result.unwrap_or_else(|error| json!({"error": error})),
            running: false,
        });
        checked.push(Checked { command, passed });
    }
    checked
}

/// The combined report the chat shows.
#[must_use]
pub fn report(graph: &Graph, summary: &Summary) -> String {
    let mut text = format!(
        "All {} parts are done{}:",
        graph.parts.len(),
        if summary.widest > 1 {
            format!(" ({} ran at the same time)", summary.widest)
        } else {
            String::new()
        }
    );
    for part in &summary.parts {
        let title = graph.parts[part.index]
            .lines()
            .next()
            .unwrap_or_default()
            .trim_end_matches(['.', ':'])
            .to_owned();
        let who = part
            .report
            .as_ref()
            .map(|report| {
                let name = report.model.as_deref().map_or_else(
                    || harness_name(&report.harness),
                    |model| format!("{} ({model})", harness_name(&report.harness)),
                );
                format!(": {name}")
            })
            .unwrap_or_default();
        let files = part
            .report
            .as_ref()
            .map(|report| report.changed_files.join(", "))
            .unwrap_or_default();
        let branch = part
            .report
            .as_ref()
            .and_then(|report| report.branch.clone())
            .unwrap_or_default();
        let how = match part.state {
            State::Merged if files.is_empty() => "finished; its changes are here".to_owned(),
            State::Merged => format!("finished; its changes are here ({files})"),
            State::Done => "finished, with no file changes".to_owned(),
            State::Conflict => format!(
                "finished, but its changes to {} clash with changes already here, so they stay on branch {branch}",
                if part.conflicts.is_empty() {
                    files.clone()
                } else {
                    part.conflicts.join(", ")
                }
            ),
            State::Held => format!(
                "finished; {} Its changes stay on branch {branch}",
                part.why.clone().unwrap_or_default()
            ),
            State::Failed => format!(
                "stopped with an error: {}",
                part.why.clone().unwrap_or_default()
            ),
            State::Stopped => "was stopped".to_owned(),
            State::Skipped => part.why.clone().unwrap_or_default(),
        };
        text.push_str(&format!("\n{}. {title}{who} ({how})", part.index + 1));
    }
    if !summary.checks.is_empty() {
        let passed = summary.checks.iter().filter(|check| check.passed).count();
        text.push_str(&format!(
            "\nChecks on the combined changes: {passed} of {} pass",
            summary.checks.len()
        ));
        for check in summary.checks.iter().filter(|check| !check.passed) {
            text.push_str(&format!("; `{}` fails", check.command));
        }
        text.push('.');
    }
    text
}

fn harness_name(id: &str) -> String {
    match id {
        crate::delegate_host::CLAUDE => "Claude Code".into(),
        crate::delegate_host::CODEX => "Codex".into(),
        crate::delegate_host::OPENCODE => "OpenCode".into(),
        crate::delegate_host::CODER => "Coder".into(),
        other => other.to_owned(),
    }
}

/// The record line for a finished fan-out.
#[must_use]
pub fn record_line(run: &Run, summary: &Summary, started_ms: u64) -> FanoutLine {
    let at_ms = atif::now_ms();
    FanoutLine {
        id: run.plan.clone(),
        at_ms,
        chat: run.chat.clone(),
        question_set: QUESTION_SET.to_owned(),
        parts: u32::try_from(run.graph.parts.len()).unwrap_or(u32::MAX),
        cap: u32::try_from(run.cap).unwrap_or(u32::MAX),
        widest: u32::try_from(summary.widest).unwrap_or(u32::MAX),
        needs: run.graph.needs.clone(),
        edges: run.graph.edges(),
        outcomes: summary
            .parts
            .iter()
            .map(|part| PartLine {
                part: u32::try_from(part.index + 1).unwrap_or(u32::MAX),
                child: part.report.as_ref().map(|report| report.child.clone()),
                harness: part.report.as_ref().map(|report| report.harness.clone()),
                state: part.state,
                files: part
                    .report
                    .as_ref()
                    .map_or(0, |report| report.changed_files.len() + report.more_files),
                conflicts: u32::try_from(part.conflicts.len()).unwrap_or(u32::MAX),
                started_ms: part.started_ms,
                ended_ms: part.ended_ms,
                error: part
                    .why
                    .as_deref()
                    .filter(|_| matches!(part.state, State::Failed))
                    .map(crate::routing_record::scrub),
            })
            .collect(),
        checks_passed: (!summary.checks.is_empty())
            .then(|| summary.checks.iter().all(|check| check.passed)),
        pin: (run.to != "auto").then(|| run.to.clone()),
        ms: at_ms.saturating_sub(started_ms),
    }
}

/// A fan-out as its record writes it (#11539): the graph and how each
/// part went.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FanoutLine {
    /// The plan's id.
    pub id: String,
    pub at_ms: u64,
    pub chat: Option<String>,
    pub question_set: String,
    pub parts: u32,
    pub cap: u32,
    /// The most parts that ran at the same time.
    pub widest: u32,
    /// Every judged pair, with its probability.
    pub needs: Vec<Need>,
    /// The edges kept: `(part, on)`.
    pub edges: Vec<(u32, u32)>,
    pub outcomes: Vec<PartLine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks_passed: Option<bool>,
    /// The harness or agent the chat was pinned to, which every part ran
    /// on (#11544); absent when Auto chose each part's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    pub ms: u64,
}

/// One part's line in a [`FanoutLine`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartLine {
    pub part: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    pub state: State,
    pub files: usize,
    pub conflicts: u32,
    pub started_ms: u64,
    pub ended_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Runs `run` on this worker with Coder's delegate host: the whole
/// fan-out, its record, and the combined report as the turn's reply.
pub async fn work(
    run: Run,
    cancel: &Arc<AtomicBool>,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<openrouter::Streamed, String> {
    let started_ms = atif::now_ms();
    let (sender, events) = tokio::sync::mpsc::unbounded_channel();
    let delegator = Arc::new(crate::delegate_host::delegator(
        &run.execution,
        crate::delegate_host::CODER,
        None,
        cancel.clone(),
        Some(sender),
    ));
    let summary = execute(&run, delegator, Some(events), cancel, emit).await;
    if let Some(record) = &run.record {
        record.fanout(record_line(&run, &summary, started_ms));
    }
    Ok(openrouter::Streamed {
        text: report(&run.graph, &summary),
        ..openrouter::Streamed::default()
    })
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    git_bytes(cwd, args).map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
}

fn git_bytes(cwd: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

fn git_input(cwd: &Path, args: &[&str], input: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut child = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input).map_err(|error| error.to_string())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// Whether `cwd` is in a Git checkout with a commit, where parts can get
/// worktrees.
#[must_use]
pub fn can_fan_out(cwd: &Path) -> bool {
    git(cwd, &["rev-parse", "--verify", "HEAD"]).is_ok()
}

impl crate::App {
    /// Whether this chat's folder can give parts worktrees.
    pub(crate) fn can_fan_out(&self) -> bool {
        can_fan_out(&self.context_cwd())
    }

    /// The harness and model each part runs on: the chat's pin (a
    /// harness, or the ACP agent picked with Shift+Tab), else `delegate`'s
    /// `auto`. A pinned chat's parts never go anywhere else (#11544).
    pub(crate) fn fanout_target(&self) -> (String, Option<String>) {
        use crate::harness_plugin::Harness;
        if self.live.pinned
            && let Some(agent) = &self.live.agent
        {
            return (agent.clone(), None);
        }
        let pinned = self.live.runs_on.filter(|_| self.live.pinned);
        let to = match pinned {
            Some(Harness::ClaudeCode) => crate::delegate_host::CLAUDE,
            Some(Harness::Codex) => crate::delegate_host::CODEX,
            Some(Harness::OpenCode) => crate::delegate_host::OPENCODE,
            Some(Harness::CoderLoop) => crate::delegate_host::CODER,
            None => "auto",
        };
        let model = pinned.and(self.live.pinned_model.clone());
        (to.to_owned(), model)
    }

    /// Starts `graph` in place of the held `message` (#11539): the message
    /// and the outline show, and the worker runs the parts, at most the
    /// person's cap at once, as one turn whose reply is the combined
    /// report.
    pub(crate) fn start_fanout(
        &mut self,
        message: String,
        plan: crate::dispatch::Plan,
        graph: Graph,
    ) {
        let cap = cap(self.preferences.parallel_parts);
        self.cancel_request();
        self.live.entries.push(crate::live::Entry::User(message));
        self.live.entries.push(crate::live::Entry::Assistant {
            text: outline(&graph, cap),
            model: None,
            elapsed_ms: None,
        });
        self.live.work_started = self.live.user_count();
        self.live.notice = None;
        self.live.partial.clear();
        self.live.partial_model = None;
        self.live.text_streamed = false;
        self.live.busy = true;
        self.live.reply_started_at = Some(std::time::Instant::now());
        self.history.dirty = true;
        self.begin_turn_record();
        let mut execution = self.plugins.execution_settings(self.context_cwd());
        execution.instructions = self.live.instructions.clone();
        execution.boundary = self.boundary.clone();
        execution.connections = self.connections();
        execution.fleet = Some(self.fleet_host());
        let (to, model) = self.fanout_target();
        let run = Run {
            plan: plan.id,
            around: plan.around,
            graph,
            cap,
            to,
            model,
            execution,
            record: self.routing_recorder(),
            chat: self.session_id().map(str::to_owned),
        };
        self.screen = crate::Screen::Conversation;
        self.scroll = u16::MAX;
        self.request = Some(crate::live::Request {
            id: self.request_id,
            key: model_access::ApiKey::new(""),
            kind: crate::live::Work::Fanout { run: Box::new(run) },
        });
    }
}

#[cfg(test)]
thread_local! {
    static TEST_NEEDS: std::cell::RefCell<Option<Vec<(u32, u32)>>> =
        const { std::cell::RefCell::new(None) };
}

/// Sets this test thread's judged edges, `(part, on)` from one; every
/// other pair is judged independent. `None` leaves the needs unanswered.
#[cfg(test)]
pub(crate) fn set_test_needs(edges: Option<Vec<(u32, u32)>>) {
    TEST_NEEDS.with(|cell| *cell.borrow_mut() = edges);
}

/// This test thread's needs for an `n`-part list.
#[cfg(test)]
pub(crate) fn test_needs(n: usize) -> Option<Vec<Need>> {
    let edges = TEST_NEEDS.with(|cell| cell.borrow().clone())?;
    let mut needs = Vec::new();
    for part in 2..=u32::try_from(n).unwrap_or(0) {
        for on in 1..part {
            needs.push(Need {
                part,
                on,
                p: if edges.contains(&(part, on)) {
                    0.9
                } else {
                    0.1
                },
            });
        }
    }
    Some(needs)
}

#[cfg(test)]
#[path = "fanout_tests.rs"]
mod tests;
