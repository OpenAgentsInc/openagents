//! The general rule engine (phase 2): when a rule's triggers fire, whether
//! its conditions hold, and the actions that are not disk cleanup
//! (notifications and keeping a Git checkout up to date). Disk cleanup
//! actions still run through [`crate::run`], with every safety check, the
//! audit log, and undo; this module records its own steps in the same log.
//!
//! Everything here takes the time and the observations as arguments, so
//! tests inject a clock, file events, and task endings.

use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::paths::{self, Layout, bytes, show};
use crate::plan::{Env, TaskFact};
use crate::rule::{self, Action, Condition, Rule, TaskOutcome, Trigger, expand};
use crate::run::{Cause, Record, Report};
use crate::store::{self, State};

pub use crate::builtins::Powers;

/// One named threshold on a Jev probability, as
/// `coder_delegate::decision::Setting` names them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setting {
    pub name: &'static str,
    pub default: f64,
}

impl Setting {
    #[must_use]
    pub const fn new(name: &'static str, default: f64) -> Self {
        Self { name, default }
    }

    /// Whether a probability reads as yes.
    #[must_use]
    pub fn yes(self, p: f64) -> bool {
        p >= self.default
    }
}

/// `background.judgment`: the default bar a `Judgment` condition's Noul
/// must reach. A condition carries its own percent; this is what the
/// compiler writes. Unmeasured.
pub const JUDGMENT: Setting = Setting::new("background.judgment", 0.8);

/// A question Jev answers over a state: a yes-or-no (Noul) or one of a
/// list of options (Choice: what to choose, then `(id, what it means)`).
#[derive(Clone, Debug, PartialEq)]
pub enum Question {
    Noul(String),
    Choice {
        instructions: String,
        options: Vec<(String, String)>,
    },
}

/// Jev's answer to one question: the Noul's probability, or each option's
/// probability, most likely first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answer {
    pub noul: Option<f64>,
    pub choice: Vec<(String, f64)>,
}

impl Answer {
    /// The most likely option and its probability.
    #[must_use]
    pub fn top(&self) -> Option<(&str, f64)> {
        self.choice
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, p)| (id.as_str(), *p))
    }
}

/// System One: asks Jev a set of typed questions about one state. The CLI
/// and the host give it the hosted Jev; tests give it a stand-in.
pub trait Judge: Send + Sync {
    /// # Errors
    /// Jev could not be asked.
    fn ask(
        &self,
        state: &str,
        questions: &[(String, Question)],
    ) -> Result<BTreeMap<String, Answer>, String>;
}

/// The local time: seconds since the epoch and the zone's offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    pub now: u64,
    /// Seconds east of UTC.
    pub offset: i64,
}

impl Clock {
    /// The computer's clock and time zone.
    #[must_use]
    pub fn here() -> Self {
        let now = paths::now();
        Self {
            now,
            offset: local_offset(now),
        }
    }

    /// The local day of the week, 0 Sunday to 6 Saturday.
    #[must_use]
    pub fn weekday(self) -> u8 {
        let local = i128::from(self.now) + i128::from(self.offset);
        // 1970-01-01 was a Thursday.
        u8::try_from((local.div_euclid(86_400) + 4).rem_euclid(7)).unwrap_or(0)
    }

    /// Minutes past local midnight.
    #[must_use]
    pub fn minute(self) -> u32 {
        let local = i128::from(self.now) + i128::from(self.offset);
        u32::try_from(local.rem_euclid(86_400) / 60).unwrap_or(0)
    }

    /// The epoch second of the most recent local `HH:MM` at or before now.
    #[must_use]
    pub fn last(self, at_minute: u32) -> u64 {
        let local = i128::from(self.now) + i128::from(self.offset);
        let midnight = local - local.rem_euclid(86_400);
        let mut slot = midnight + i128::from(at_minute) * 60;
        if slot > local {
            slot -= 86_400;
        }
        u64::try_from(slot - i128::from(self.offset)).unwrap_or(0)
    }

    /// The next local midnight, as an epoch second.
    #[must_use]
    pub fn next_midnight(self) -> u64 {
        self.last(0) + 86_400
    }
}

/// The local zone's offset from UTC at `at`, in seconds.
#[must_use]
pub fn local_offset(at: u64) -> i64 {
    let time = libc::time_t::try_from(at).unwrap_or(0);
    let mut tm = std::mem::MaybeUninit::<libc::tm>::zeroed();
    // SAFETY: `time` is a valid time_t and `tm` a writable `tm` that
    // `localtime_r` fills; it returns null on failure.
    let filled = unsafe { libc::localtime_r(&raw const time, tm.as_mut_ptr()) };
    if filled.is_null() {
        return 0;
    }
    // SAFETY: localtime_r succeeded, so `tm` is initialized.
    i64::from(unsafe { tm.assume_init() }.tm_gmtoff)
}

/// Whether a `Daily { at }` trigger is due: a scheduled time passed since
/// the rule last ran on it. `last` is `None` the first time the runner
/// sees the rule, which only sets the baseline (a rule saved at 15:00
/// with `09:00` first runs tomorrow). A run missed while the computer
/// slept is due once, at the first look after it.
#[must_use]
pub fn daily_due(at: &str, last: Option<u64>, clock: Clock) -> bool {
    let Some(minute) = rule::clock(at) else {
        return false;
    };
    last.is_some_and(|last| clock.last(minute) > last)
}

/// The next interval check: `every` plus a jitter of up to 10%, chosen
/// from the rule's id and the time so rules started together spread out.
#[must_use]
pub fn next_interval(id: &str, every: u64, now: u64) -> u64 {
    let spread = every / 10;
    if spread == 0 {
        return now + every;
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in id.bytes().chain(now.to_le_bytes()) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    now + every + hash % (spread + 1)
}

/// What a file trigger saw of one path: whether it exists, its size, and
/// its modification time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seen {
    pub exists: bool,
    pub size: u64,
    pub modified: i64,
}

/// Look at a path now; links are not followed.
#[must_use]
pub fn see(path: &Path) -> Seen {
    std::fs::symlink_metadata(path).map_or_else(
        |_| Seen::default(),
        |meta| Seen {
            exists: true,
            size: meta.len(),
            modified: meta.mtime() * 1_000_000_000 + meta.mtime_nsec(),
        },
    )
}

/// The watched paths that changed between two looks. A path seen for the
/// first time is a baseline, not a change.
#[must_use]
pub fn changed(before: &BTreeMap<String, Seen>, now: &BTreeMap<String, Seen>) -> Vec<String> {
    now.iter()
        .filter(|(path, seen)| before.get(*path).is_some_and(|was| was != *seen))
        .map(|(path, _)| path.clone())
        .collect()
}

/// What triggered one evaluation, beyond its cause.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Event {
    /// The task whose end this is (`TaskEnded`).
    pub task: Option<TaskFact>,
    /// The watched paths that changed (`FsEvent`).
    pub paths: Vec<String>,
}

/// How a task ended, from its fact.
#[must_use]
pub fn outcome(task: &TaskFact) -> TaskOutcome {
    if task.cancelled {
        TaskOutcome::Cancelled
    } else if task.failed {
        TaskOutcome::Failed
    } else {
        TaskOutcome::Succeeded
    }
}

/// The free space and size of the fullest watched volume.
#[must_use]
pub fn fullest(env: &Env<'_>, rule: &Rule) -> Option<(u64, u64)> {
    crate::plan::observe(env, rule)
        .iter()
        .map(|volume| (volume.space.free, volume.space.total))
        .min_by_key(|(free, _)| *free)
}

/// Whether every condition holds, or the first reason one does not.
///
/// # Errors
/// The condition that does not hold, in plain words.
pub fn holds(
    env: &Env<'_>,
    rule: &Rule,
    event: &Event,
    clock: Clock,
    judge: Option<&dyn Judge>,
) -> Result<(), String> {
    for condition in &rule.conditions {
        match condition {
            Condition::FreeBelow { level } => {
                let Some((free, total)) = fullest(env, rule) else {
                    return Err("free space could not be read".into());
                };
                let bound = level.of(total);
                if free >= bound {
                    return Err(format!("{} free, not below {}", bytes(free), bytes(bound)));
                }
            }
            Condition::TaskOutcome { outcomes } => {
                let Some(task) = &event.task else {
                    return Err("no Coder run has just ended".into());
                };
                let ended = outcome(task);
                if !outcomes.contains(&ended) {
                    return Err(format!("task {} {}", short(&task.id), said(ended)));
                }
            }
            Condition::NoTaskRunning => {
                let running = env
                    .facts
                    .map(|facts| facts.tasks())
                    .transpose()
                    .map_err(|why| format!("the task store could not be read: {why}"))?
                    .unwrap_or_default()
                    .iter()
                    .filter(|task| task.running)
                    .count();
                if running > 0 {
                    return Err(format!("{running} Coder task(s) running"));
                }
            }
            Condition::PathExists { path } => {
                if std::fs::symlink_metadata(expand(path, &env.layout.home)).is_err() {
                    return Err(format!("{path} does not exist"));
                }
            }
            Condition::TimeBetween { from, to } => {
                let (Some(from_m), Some(to_m)) = (rule::clock(from), rule::clock(to)) else {
                    return Err("the time span does not read".into());
                };
                let minute = clock.minute();
                let inside = if from_m <= to_m {
                    (from_m..to_m).contains(&minute)
                } else {
                    minute >= from_m || minute < to_m
                };
                if !inside {
                    return Err(format!("it is not between {from} and {to}"));
                }
            }
            Condition::Weekdays { days } => {
                let today = clock.weekday();
                if !days.contains(&today) {
                    return Err(format!("it is {}", crate::rule::day_name(today)));
                }
            }
            Condition::Judgment {
                question,
                setting,
                threshold,
            } => {
                let Some(judge) = judge else {
                    return Err(format!("{setting}: no judge is set up here"));
                };
                let state = judgment_state(env, rule, event, clock);
                let answers = judge.ask(
                    &state,
                    &[("judgment".to_owned(), Question::Noul(question.clone()))],
                )?;
                let p = answers
                    .get("judgment")
                    .and_then(|answer| answer.noul)
                    .ok_or("the judge did not answer")?;
                if p < f64::from(*threshold) / 100.0 {
                    return Err(format!(
                        "{setting}: {:.2} below {:.2}",
                        p,
                        f64::from(*threshold) / 100.0
                    ));
                }
            }
        }
    }
    Ok(())
}

/// The state a judgment reads: code-built facts, never a transcript.
fn judgment_state(env: &Env<'_>, rule: &Rule, event: &Event, clock: Clock) -> String {
    let mut lines = vec![format!("Rule: {}", rule.name)];
    if let Some((free, total)) = fullest(env, rule) {
        lines.push(format!(
            "Free disk space: {} of {}",
            bytes(free),
            bytes(total)
        ));
    }
    if let Some(task) = &event.task {
        lines.push(format!(
            "A Coder task just ended: {} ({})",
            task.id,
            said(outcome(task))
        ));
    }
    for path in &event.paths {
        lines.push(format!("Changed: {path}"));
    }
    lines.push(format!(
        "Local time: {:02}:{:02}",
        clock.minute() / 60,
        clock.minute() % 60
    ));
    lines.join("\n")
}

fn said(outcome: TaskOutcome) -> &'static str {
    match outcome {
        TaskOutcome::Succeeded => "finished",
        TaskOutcome::Failed => "failed",
        TaskOutcome::Cancelled => "was cancelled",
    }
}

fn short(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

/// One step a rule took that is not a deletion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// `notify` or `git_fast_forward`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub outcome: StepOutcome,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepOutcome {
    Done,
    /// It would have, in a dry run.
    Would,
    /// Nothing to do, or a check said no.
    Skipped,
    /// Left as it is because it needs the person (a checkout with
    /// uncommitted work, or on another branch): said once, not every check.
    Blocked,
    Failed,
}

/// A notification's text with its placeholders filled: `{task}` (the
/// ended task's short id), `{outcome}`, and `{free}`.
#[must_use]
pub fn fill(text: &str, env: &Env<'_>, rule: &Rule, event: &Event) -> String {
    let mut out = text.to_owned();
    if let Some(task) = &event.task {
        out = out
            .replace("{task}", short(&task.id))
            .replace("{outcome}", said(outcome(task)));
    }
    if out.contains("{free}") {
        let free = fullest(env, rule).map_or_else(|| "unknown".to_owned(), |(f, _)| bytes(f));
        out = out.replace("{free}", &free);
    }
    out
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

/// Bring `repo` up to date with its upstream, only by fast-forward and
/// only when it is clean and has nothing its upstream lacks. A dry run
/// fetches nothing and reads the last-fetched upstream.
#[must_use]
pub fn fast_forward(repo: &str, branch: Option<&str>, home: &Path, dry_run: bool) -> Step {
    let path = expand(repo, home);
    let shown = show(&path, home);
    let step = |outcome, detail: String| Step {
        kind: "git_fast_forward".into(),
        target: Some(shown.clone()),
        outcome,
        detail,
    };
    if git(&path, &["rev-parse", "--is-inside-work-tree"]).as_deref() != Ok("true") {
        return step(
            StepOutcome::Skipped,
            format!("{shown} is not a Git checkout"),
        );
    }
    if let Some(wanted) = branch {
        let on = git(&path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
        if on != wanted {
            return step(
                StepOutcome::Blocked,
                format!("{shown} is on {on}, not {wanted}; left as it is."),
            );
        }
    }
    match git(&path, &["status", "--porcelain"]) {
        Ok(status) if !status.is_empty() => {
            return step(
                if branch.is_some() {
                    StepOutcome::Blocked
                } else {
                    StepOutcome::Skipped
                },
                format!("{shown} has uncommitted changes; left as it is."),
            );
        }
        Err(why) => return step(StepOutcome::Failed, why),
        Ok(_) => {}
    }
    let Ok(upstream) = git(&path, &["rev-parse", "--abbrev-ref", "@{u}"]) else {
        return step(
            StepOutcome::Skipped,
            format!("{shown}'s branch tracks no remote branch"),
        );
    };
    if !dry_run && let Err(why) = git(&path, &["fetch", "--quiet"]) {
        return step(StepOutcome::Failed, format!("fetch failed: {why}"));
    }
    let count = |range: &str| {
        git(&path, &["rev-list", "--count", range])
            .ok()
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let ahead = count("@{u}..HEAD");
    if ahead > 0 {
        return step(
            if branch.is_some() {
                StepOutcome::Blocked
            } else {
                StepOutcome::Skipped
            },
            format!("{shown} has {ahead} commit(s) {upstream} lacks; left as it is"),
        );
    }
    let behind = count("HEAD..@{u}");
    if dry_run {
        return step(
            StepOutcome::Would,
            format!(
                "{shown} is clean; would fetch and fast-forward to {upstream} ({behind} commit(s) behind as of its last fetch)"
            ),
        );
    }
    if behind == 0 {
        return step(
            StepOutcome::Skipped,
            format!("{shown} is up to date with {upstream}"),
        );
    }
    match git(&path, &["merge", "--ff-only", "--quiet", "@{u}"]) {
        Ok(_) => step(
            StepOutcome::Done,
            format!("Updated {shown} to {upstream}: {behind} new commit(s)."),
        ),
        Err(why) => step(StepOutcome::Failed, format!("fast-forward failed: {why}")),
    }
}

/// Run a rule's actions that are not disk cleanup, after its conditions
/// hold. A dry run says what each would do and changes nothing.
#[must_use]
pub fn steps(env: &Env<'_>, rule: &Rule, event: &Event, dry_run: bool) -> Vec<Step> {
    steps_with(env, rule, event, Powers::default(), dry_run)
}

/// [`steps`] with the host's judge and services for the phase 3 actions.
#[must_use]
pub fn steps_with(
    env: &Env<'_>,
    rule: &Rule,
    event: &Event,
    powers: Powers<'_>,
    dry_run: bool,
) -> Vec<Step> {
    let mut out = Vec::new();
    for action in &rule.actions {
        if let Some(steps) = crate::builtins::steps(env, rule, action, powers, dry_run) {
            out.extend(steps);
        } else {
            out.extend(plain(env, rule, event, action, dry_run));
        }
    }
    out
}

fn plain(
    env: &Env<'_>,
    rule: &Rule,
    event: &Event,
    action: &Action,
    dry_run: bool,
) -> Option<Step> {
    match action {
        Action::Notify { text } => {
            let text = fill(text, env, rule, event);
            Some(Step {
                kind: "notify".into(),
                target: None,
                outcome: if dry_run {
                    StepOutcome::Would
                } else {
                    StepOutcome::Done
                },
                detail: text,
            })
        }
        Action::GitFastForward { repo, branch } => Some(fast_forward(
            repo,
            branch.as_deref(),
            &env.layout.home,
            dry_run,
        )),
        _ => None,
    }
}

/// Evaluate a rule now: conditions, then its actions (disk cleanup
/// through [`crate::run::run`], the rest as [`steps`]). `Ok(None)` when a
/// condition does not hold. A real run is recorded in the audit log.
///
/// # Errors
/// Another run holds the run lock.
pub fn evaluate(
    env: &Env<'_>,
    rule: &Rule,
    cause: Cause,
    event: &Event,
    clock: Clock,
    judge: Option<&dyn Judge>,
    dry_run: bool,
) -> Result<Option<Report>, String> {
    evaluate_with(
        env,
        rule,
        cause,
        event,
        clock,
        Powers {
            judge,
            services: None,
        },
        dry_run,
    )
}

/// [`evaluate`] with the host's services for the phase 3 actions.
///
/// # Errors
/// Another run holds the run lock.
pub fn evaluate_with(
    env: &Env<'_>,
    rule: &Rule,
    cause: Cause,
    event: &Event,
    clock: Clock,
    powers: Powers<'_>,
    dry_run: bool,
) -> Result<Option<Report>, String> {
    let judge = powers.judge;
    if holds(env, rule, event, clock, judge).is_err() {
        return Ok(None);
    }
    let mut report = if rule.cleans() {
        Some(crate::run::run(
            env,
            rule,
            cause,
            dry_run,
            cause == Cause::Manual,
        )?)
    } else {
        None
    };
    let steps = steps_with(env, rule, event, powers, dry_run);
    if steps.is_empty() {
        return Ok(report);
    }
    let blocked: Option<String> = {
        let lines: Vec<&str> = steps
            .iter()
            .filter(|step| step.outcome == StepOutcome::Blocked)
            .map(|step| step.detail.as_str())
            .collect();
        (!lines.is_empty()).then(|| lines.join(" "))
    };
    let said_before = State::load(env.layout)
        .rules
        .get(&rule.id)
        .and_then(|state| state.last_blocked.clone());
    if !dry_run && blocked != said_before {
        let now_blocked = blocked.clone();
        State::update(env.layout, &rule.id, |state| {
            state.last_blocked = now_blocked
        });
    }
    let repeat = blocked.is_some() && blocked == said_before;
    let said: Vec<String> = steps
        .iter()
        .filter(|step| step.outcome != StepOutcome::Would)
        .filter(|step| step.kind == "notify" || step.outcome != StepOutcome::Skipped)
        .filter(|step| !(repeat && step.outcome == StepOutcome::Blocked))
        .map(|step| step.detail.clone())
        .collect();
    let line = (!said.is_empty()).then(|| said.join(" "));
    let started = env.now;
    match &mut report {
        Some(report) => {
            if let Some(record) = &mut report.record {
                record.steps.extend(steps.clone());
            }
            report.notice = match (report.notice.take(), line) {
                (Some(a), Some(b)) => Some(format!("{a} {b}")),
                (a, b) => a.or(b),
            };
        }
        None => {
            let record = (!dry_run).then(|| {
                let mut record = Record::empty(&crate::run::new_id(started), &rule.id);
                record.rule_version = rule.version;
                record.rule_digest = rule.digest();
                record.trigger = cause;
                record.started = started;
                record.ended = paths::now();
                record.steps = steps.clone();
                record.notified.clone_from(&line);
                store::append(env.layout, &record);
                record
            });
            report = Some(Report {
                dry_run,
                plan: crate::plan::Plan::default(),
                record,
                notice: line,
                steps: if dry_run { steps } else { Vec::new() },
            });
        }
    }
    Ok(report)
}

/// The rule's ready-to-show dry run: what it would do if it fired now.
#[must_use]
pub fn dry_run(env: &Env<'_>, rule: &Rule, clock: Clock) -> Vec<String> {
    let mut lines = Vec::new();
    let event = Event::default();
    let waits_for_task = rule
        .conditions
        .iter()
        .any(|c| matches!(c, Condition::TaskOutcome { .. }));
    if waits_for_task {
        lines.push("Nothing to try now: it acts when a Coder run ends.".to_owned());
        return lines;
    }
    if rule.cleans() {
        match crate::run::run(env, rule, Cause::Manual, true, false) {
            Ok(report) => {
                let planned: u64 = report.plan.volumes.iter().map(|v| v.planned()).sum();
                if planned == 0 {
                    if let Some((free, total)) = fullest(env, rule) {
                        lines.push(format!(
                            "Nothing to clean now: {} free, above {}.",
                            bytes(free),
                            bytes(rule.goal.start.of(total))
                        ));
                    }
                } else {
                    lines.extend(crate::run::describe(&report.plan, &env.layout.home, false));
                }
            }
            Err(why) => lines.push(why),
        }
    }
    match holds(env, rule, &event, clock, None) {
        Err(why) if !rule.conditions.is_empty() => {
            lines.push(format!("Now it would do nothing: {why}."));
        }
        _ => {
            for step in steps_with(env, rule, &event, Powers::default(), true) {
                lines.push(match step.outcome {
                    StepOutcome::Would if step.kind == "notify" => {
                        format!("Now it would tell you: {}", step.detail)
                    }
                    _ => format!("Now: {}", step.detail),
                });
            }
        }
    }
    lines
}

/// The paths a rule's file triggers watch, expanded.
#[must_use]
pub fn watched(rule: &Rule) -> Vec<String> {
    rule.triggers
        .iter()
        .filter_map(|trigger| match trigger {
            Trigger::FsEvent { paths } => Some(paths.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// Look at every watched path of `rules`, compare with the last look
/// (`watched.json`), save this one, and return which rules saw a change
/// and the paths that changed.
#[must_use]
pub fn poll_files(layout: &Layout, rules: &[Rule]) -> Vec<(String, Vec<String>)> {
    let before: BTreeMap<String, Seen> = std::fs::read(layout.watched())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let mut now = BTreeMap::new();
    for rule in rules {
        for path in watched(rule) {
            now.insert(path.clone(), see(&expand(&path, &layout.home)));
        }
    }
    let moved = changed(&before, &now);
    if now != before
        && let Ok(bytes) = serde_json::to_vec(&now)
    {
        let _ = store::write_atomic(&layout.watched(), &bytes);
    }
    rules
        .iter()
        .filter_map(|rule| {
            let paths: Vec<String> = watched(rule)
                .into_iter()
                .filter(|path| moved.contains(path))
                .collect();
            (!paths.is_empty()).then(|| (rule.id.clone(), paths))
        })
        .collect()
}

/// Remember a daily run (or the baseline the first time).
pub fn mark_daily(layout: &Layout, id: &str, at: u64) {
    State::update(layout, id, |state| state.last_daily = Some(at));
}

/// Whether a rule reads the outcome of the task whose end triggered it,
/// so each ended task is its own evaluation.
#[must_use]
pub fn per_task(rule: &Rule) -> bool {
    rule.conditions
        .iter()
        .any(|condition| matches!(condition, Condition::TaskOutcome { .. }))
}

/// A report of a run that did nothing.
#[must_use]
pub fn nothing() -> Report {
    Report {
        dry_run: false,
        plan: crate::plan::Plan::default(),
        record: None,
        notice: None,
        steps: Vec::new(),
    }
}
