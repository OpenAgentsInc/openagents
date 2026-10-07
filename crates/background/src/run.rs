//! One run: plan, check each item again, delete, measure, record, and say
//! what happened in one line.

use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::git::{self, Undo};
use crate::paths::{self, Layout, bytes, show};
use crate::plan::{self, Env, Evidence, Item, Kept, Plan, View};
use crate::rule::{Class, Rule};
use crate::store;

/// What started a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    Interval,
    Threshold,
    TaskEnded,
    HostStart,
    /// A `Daily` trigger's time passed.
    Daily,
    /// A watched path changed.
    FsEvent,
    /// Someone asked: `openagents background run`, `/background`, or
    /// `background.run`.
    Manual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Deleted,
    /// A worktree removed with `git worktree remove`.
    Removed,
    /// kache's own collector ran on its store.
    Collected,
    /// Moved to the background trash (a confirmed cache), emptied after
    /// 24 hours; `undo` puts it back.
    Trashed,
    /// A check failed right before deletion.
    Skipped,
    Failed,
}

/// One action of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    pub class: Class,
    pub path: PathBuf,
    /// Allocated bytes, measured right before deletion.
    pub bytes: u64,
    pub outcome: Outcome,
    pub reason: String,
    pub evidence: Evidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo: Option<Undo>,
    /// Where a trashed folder went.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trashed: Option<PathBuf>,
}

/// One volume before and after.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub volume: PathBuf,
    pub total: u64,
    pub free_before: u64,
    pub free_after: u64,
}

/// A run in the audit log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub run: String,
    pub rule: String,
    pub rule_version: u64,
    pub rule_digest: String,
    pub trigger: Cause,
    pub started: u64,
    pub ended: u64,
    pub emergency: bool,
    pub observation: Vec<Observation>,
    pub actions: Vec<Action>,
    /// The sum of what was deleted.
    pub freed_sum: u64,
    /// The measured rise in free space.
    pub freed_measured: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notified: Option<String>,
    pub escalated: bool,
    /// What the rule did that is not a deletion: notifications and
    /// checkout updates (phase 2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<crate::engine::Step>,
    /// Folders Jev judged after the run fell short (phase 3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub judgments: Vec<crate::judged::Judgment>,
}

impl Record {
    #[must_use]
    pub fn empty(run: &str, rule: &str) -> Self {
        Self {
            run: run.into(),
            rule: rule.into(),
            rule_version: 0,
            rule_digest: String::new(),
            trigger: Cause::Manual,
            started: 0,
            ended: 0,
            emergency: false,
            observation: Vec::new(),
            actions: Vec::new(),
            freed_sum: 0,
            freed_measured: 0,
            notified: None,
            escalated: false,
            steps: Vec::new(),
            judgments: Vec::new(),
        }
    }
}

/// What [`run`] did, or would do for a dry run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub dry_run: bool,
    pub plan: Plan,
    /// `None` for a dry run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<Record>,
    /// The one line, when there is anything to say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    /// A dry run's steps that are not deletions (a real run's are in its
    /// record).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<crate::engine::Step>,
}

/// A new run's id.
#[must_use]
pub fn new_id(now: u64) -> String {
    run_id(now)
}

fn run_id(now: u64) -> String {
    static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{now}-{}-{n}", std::process::id())
}

/// Hold the run lock, so two runs never delete at once.
///
/// # Errors
/// Another run holds it.
pub fn lock(layout: &Layout) -> Result<File, String> {
    std::fs::create_dir_all(layout.background()).map_err(|error| error.to_string())?;
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(layout.run_lock())
        .map_err(|error| error.to_string())?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => Err("A cleanup is already running.".into()),
        Err(std::fs::TryLockError::Error(error)) => Err(error.to_string()),
    }
}

/// Plan, and unless `dry_run`, execute and record. `force` plans for any
/// volume below its stop level (a run someone asked for); triggers plan
/// only for volumes below their start level.
///
/// # Errors
/// Another run holds the run lock.
pub fn run(
    env: &Env<'_>,
    rule: &Rule,
    cause: Cause,
    dry_run: bool,
    force: bool,
) -> Result<Report, String> {
    let plan = plan::plan(env, rule, force);
    if dry_run {
        return Ok(Report {
            dry_run,
            plan,
            record: None,
            notice: None,
            steps: Vec::new(),
        });
    }
    let _lock = lock(env.layout)?;
    let expired = expire_trash(env);
    Ok(execute_after(env, rule, cause, plan, expired))
}

/// How long a trashed folder stays.
pub const TRASH_SECS: u64 = 24 * 3600;

/// Delete what has been in the background trash past its window.
fn expire_trash(env: &Env<'_>) -> Vec<Action> {
    let mut done = Vec::new();
    let Ok(entries) = std::fs::read_dir(env.layout.trash()) else {
        return done;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        let at = meta.modified().map_or(0, paths::unix);
        if env.now.saturating_sub(at) < TRASH_SECS {
            continue;
        }
        let bytes = paths::measure(&path, &env.layout.home).map_or(0, |m| m.bytes);
        let outcome = std::fs::remove_dir_all(&path);
        done.push(Action {
            class: Class::Trash,
            path: path.clone(),
            bytes: if outcome.is_ok() { bytes } else { 0 },
            outcome: if outcome.is_ok() {
                Outcome::Deleted
            } else {
                Outcome::Failed
            },
            reason: outcome.map_or_else(
                |error| error.to_string(),
                |()| "in the trash past 24 hours".into(),
            ),
            evidence: Evidence::default(),
            undo: None,
            trashed: None,
        });
    }
    done
}

/// Execute `plan`: each item is checked again (locks taken and held,
/// processes read fresh, the task store read fresh) right before it is
/// deleted.
pub fn execute(env: &Env<'_>, rule: &Rule, cause: Cause, plan: Plan) -> Report {
    execute_after(env, rule, cause, plan, Vec::new())
}

/// [`execute`], recording `before` (the trash past its window) first.
fn execute_after(
    env: &Env<'_>,
    rule: &Rule,
    cause: Cause,
    plan: Plan,
    before: Vec<Action>,
) -> Report {
    let started = paths::now();
    let run = run_id(started);
    let mut actions = before;
    for item in plan.items() {
        actions.push(act(env, rule, item, &run));
    }
    let freed_sum: u64 = actions
        .iter()
        .filter(|action| {
            matches!(
                action.outcome,
                Outcome::Deleted | Outcome::Removed | Outcome::Collected
            )
        })
        .map(|action| action.bytes)
        .sum();
    let mut observation = Vec::new();
    for volume in &plan.volumes {
        let after = env
            .volumes
            .space(&volume.root)
            .map_or(volume.space.free, |space| space.free);
        observation.push(Observation {
            volume: volume.root.clone(),
            total: volume.space.total,
            free_before: volume.space.free,
            free_after: after,
        });
    }
    let measured: i64 = observation
        .iter()
        .map(|o| {
            i64::try_from(o.free_after).unwrap_or(i64::MAX)
                - i64::try_from(o.free_before).unwrap_or(i64::MAX)
        })
        .sum();
    let emergency = plan.volumes.iter().any(|volume| volume.emergency);
    let mut plan = plan;
    let low = plan
        .volumes
        .iter()
        .zip(&observation)
        .any(|(volume, o)| o.free_after < volume.start && volume.needed > 0);
    if low && plan.not_cleaned.is_empty() {
        plan.not_cleaned = plan::not_cleaned(env.layout, rule);
    }
    let notice = notice(
        env.layout,
        &plan,
        &actions,
        &observation,
        freed_sum,
        measured,
        emergency,
    );
    let record = Record {
        run,
        rule: rule.id.clone(),
        rule_version: rule.version,
        rule_digest: rule.digest(),
        trigger: cause,
        started,
        ended: paths::now(),
        emergency,
        observation,
        actions,
        freed_sum,
        freed_measured: measured,
        notified: notice.clone(),
        escalated: false,
        steps: Vec::new(),
        judgments: Vec::new(),
    };
    if !record.actions.is_empty() || record.notified.is_some() {
        store::append(env.layout, &record);
    }
    Report {
        dry_run: false,
        plan,
        record: Some(record),
        notice,
        steps: Vec::new(),
    }
}

fn act(env: &Env<'_>, rule: &Rule, item: &Item, run: &str) -> Action {
    let mut action = Action {
        class: item.class,
        path: item.path.clone(),
        bytes: 0,
        outcome: Outcome::Skipped,
        reason: item.why.clone(),
        evidence: item.evidence.clone(),
        undo: None,
        trashed: None,
    };
    if item.class == Class::Kache {
        collect(env, &mut action);
        return action;
    }
    let touched = match item.class {
        Class::Worktrees | Class::Trash => paths::touched_worktree(&item.path),
        Class::ClaudeWorktrees => paths::touched_linked(&item.path),
        Class::Judged => plan::newest(&item.path),
        _ => paths::touched(&item.path),
    };
    if touched > item.touched && item.class != Class::Incremental {
        action.reason = "used since the plan".into();
        return action;
    }
    let view = View::read(env);
    let snapshot = match env.processes.snapshot() {
        Ok(snapshot) => snapshot,
        Err(why) => {
            action.reason = format!("cannot tell what is in use: {why}");
            return action;
        }
    };
    let (held, undo, _) = match plan::check(
        env.layout,
        rule,
        &view,
        &snapshot,
        item.class,
        &item.path,
        &item.locks,
    ) {
        Ok(checked) => checked,
        Err(why) => {
            action.reason = why;
            return action;
        }
    };
    action.bytes = paths::measure(&item.path, &env.layout.home).map_or(item.bytes, |m| m.bytes);
    let trash = (item.class == Class::Judged).then(|| trash_path(env.layout, run, &item.path));
    let result = match (&undo, &trash) {
        (Some(undo), _) => git::remove(undo).map(|()| Outcome::Removed),
        (None, Some(to)) => to
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::rename(&item.path, to))
            .map(|()| Outcome::Trashed)
            .map_err(|error| error.to_string()),
        (None, None) => std::fs::remove_dir_all(&item.path)
            .map(|()| Outcome::Deleted)
            .map_err(|error| error.to_string()),
    };
    drop(held);
    match result {
        Ok(outcome) => {
            action.outcome = outcome;
            action.undo = undo;
            if outcome == Outcome::Trashed {
                action.trashed = trash;
                if let Some(at) = action.trashed.as_ref().and_then(|p| p.parent()) {
                    // The trash window starts now.
                    let _ = std::fs::File::open(at)
                        .and_then(|dir| dir.set_modified(std::time::SystemTime::now()));
                }
            }
        }
        Err(_) if trash.is_some() => {
            action.outcome = Outcome::Failed;
            action.reason = "could not move it to the trash (another volume?)".into();
            action.bytes = 0;
        }
        Err(why) => {
            action.outcome = Outcome::Failed;
            action.reason = why;
            // A partial deletion still freed something; measure what is left.
            let left = paths::measure(&item.path, &env.layout.home).map_or(0, |m| m.bytes);
            action.bytes = action.bytes.saturating_sub(left);
        }
    }
    action
}

/// Run kache's own collector for a planned kache item. Nothing under the
/// store is deleted here; kache decides what to drop.
fn collect(env: &Env<'_>, action: &mut Action) {
    let Some(kache) = env.kache else {
        action.reason = "kache is not set up here".into();
        return;
    };
    match kache.reclaim() {
        Ok(report) if report.collected => {
            action.outcome = Outcome::Collected;
            action.bytes = report.disk_bytes_reclaimed;
            action.reason = format!(
                "kache's collector dropped {} entries; the store is {} of its {} cap",
                report.entries_dropped,
                bytes(report.after.store_bytes),
                bytes(report.after.store_limit_bytes)
            );
        }
        Ok(_) => action.reason = "another kache collector held the lock".into(),
        Err(why) => {
            action.outcome = Outcome::Failed;
            action.reason = why;
        }
    }
}

/// Where a confirmed cache goes in the trash: `trash/<run>/<n>-<name>`.
fn trash_path(layout: &Layout, run: &str, path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(|| "folder".into(), |n| n.to_string_lossy().into_owned());
    let dir = layout.trash().join(run);
    let mut n = 0;
    loop {
        let to = dir.join(format!("{n}-{name}"));
        if !to.exists() {
            return to;
        }
        n += 1;
    }
}

/// The one line a run says, or `None` when it did nothing worth saying.
fn notice(
    layout: &Layout,
    plan: &Plan,
    actions: &[Action],
    observation: &[Observation],
    freed: u64,
    measured: i64,
    emergency: bool,
) -> Option<String> {
    let done: Vec<&Action> = actions
        .iter()
        .filter(|action| {
            matches!(
                action.outcome,
                Outcome::Deleted | Outcome::Removed | Outcome::Trashed | Outcome::Collected
            )
        })
        .collect();
    let free = observation.iter().map(|o| o.free_after).min()?;
    let low = plan
        .volumes
        .iter()
        .zip(observation)
        .any(|(volume, o)| o.free_after < volume.start && volume.needed > 0);
    if emergency {
        return Some(format!(
            "Disk almost full ({} free). Cleaned everything allowed: {}.",
            bytes(free),
            bytes(freed)
        ));
    }
    let mut line = String::new();
    if !done.is_empty() {
        let mut counts: Vec<(Class, usize)> = Vec::new();
        for action in &done {
            match counts.iter_mut().find(|(class, _)| *class == action.class) {
                Some((_, count)) => *count += 1,
                None => counts.push((action.class, 1)),
            }
        }
        let parts: Vec<String> = counts
            .iter()
            .map(|(class, count)| format!("{count} {}", class.noun(*count)))
            .collect();
        line = if freed > 0 || counts.iter().any(|(class, _)| *class != Class::Judged) {
            format!("Freed {}: {}.", bytes(freed), parts.join(", "))
        } else {
            format!(
                "Moved {} to the trash; it empties in a day.",
                parts.join(", ")
            )
        };
        let rise = u64::try_from(measured.max(0)).unwrap_or(0);
        if cfg!(target_os = "macos") && freed > 10 * crate::rule::GB && rise < freed / 2 {
            line.push_str(&format!(
                " Free space rose only {}; local snapshots probably hold the rest.",
                bytes(rise)
            ));
        }
        if !low {
            line.push_str(&format!(" {} free.", bytes(free)));
        }
    }
    if low {
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&format!("Disk still low: {} free.", bytes(free)));
        if let Some((path, size)) = plan.not_cleaned.first() {
            line.push_str(&format!(
                " Largest not cleaned: {} ({}), not a known cache.",
                show(path, &layout.home),
                bytes(*size)
            ));
        }
    }
    (!line.is_empty()).then_some(line)
}

/// Recreate the worktrees run `run` removed.
///
/// # Errors
/// No such run.
pub fn undo(layout: &Layout, run: &str) -> Result<Vec<(PathBuf, Result<(), String>)>, String> {
    let record = store::read_log(layout)
        .into_iter()
        .rev()
        .find(|record| record.run == run)
        .ok_or_else(|| format!("no run `{run}` in the log"))?;
    Ok(record
        .actions
        .iter()
        .filter_map(|action| match (&action.undo, &action.trashed) {
            (Some(undo), _) => Some((undo.path.clone(), git::restore(undo))),
            (None, Some(trashed)) => Some((action.path.clone(), untrash(trashed, &action.path))),
            (None, None) => None,
        })
        .collect())
}

/// Put a trashed folder back where it was.
fn untrash(trashed: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        return Err(format!("{} exists again", to.display()));
    }
    if !trashed.exists() {
        return Err("the trash was emptied".into());
    }
    std::fs::rename(trashed, to).map_err(|error| error.to_string())
}

/// A plan's lines for a person: each item with class, size, and why;
/// then what is kept.
#[must_use]
pub fn describe(plan: &Plan, home: &Path, kept: bool) -> Vec<String> {
    let mut lines = Vec::new();
    if plan.volumes.iter().all(|volume| volume.items.is_empty()) {
        lines.push("Nothing qualifies for cleanup now.".to_owned());
    }
    for volume in &plan.volumes {
        // A pruning rule cleans whatever qualifies, whatever the free
        // space: its levels are the `ALWAYS` sentinel, not words for anyone.
        if volume.start >= crate::rule::ALWAYS {
            lines.push(format!(
                "{}: {} free of {}; removes whatever qualifies, whatever the free space.",
                show(&volume.root, home),
                bytes(volume.space.free),
                bytes(volume.space.total),
            ));
        } else {
            lines.push(format!(
                "{}: {} free of {}; cleans below {}, stops at {}{}.",
                show(&volume.root, home),
                bytes(volume.space.free),
                bytes(volume.space.total),
                bytes(volume.start),
                bytes(volume.stop),
                if volume.needed == 0 {
                    ", nothing needed now".to_owned()
                } else {
                    format!("; aims to free {}", bytes(volume.needed))
                }
            ));
        }
        for item in &volume.items {
            lines.push(format!(
                "  {} {:>7}  {}  ({})",
                item.class.number(),
                bytes(item.bytes),
                show(&item.path, home),
                item.why
            ));
        }
        if volume.needed > 0 {
            lines.push(format!("  Total: {}", bytes(volume.planned())));
        }
    }
    if kept {
        for Kept { class, path, why } in &plan.kept {
            lines.push(format!(
                "  kept {}  {}  ({why})",
                class.number(),
                show(path, home)
            ));
        }
    }
    for (path, size) in &plan.not_cleaned {
        lines.push(format!(
            "  not cleaned  {} {}  (not a known cache)",
            show(path, home),
            bytes(*size)
        ));
    }
    lines
}
