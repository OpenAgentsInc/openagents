//! Candidates, the checks each one passes, and the plan: what a run would
//! delete, in class order and least recently used first, until the goal.
//!
//! The same [`check`] runs while planning and again immediately before
//! each deletion ([`crate::run`]), so a dry run shows exactly what a real
//! run would do at that moment.

use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::git::{self, Undo};
use crate::inuse::{self, Held, Processes, Snapshot};
use crate::paths::{self, Layout, SLOTS, real_dir};
use crate::rule::{Class, Rule, glob};
use crate::volume::{Space, Volumes};

/// What the task store says about one Coder task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFact {
    pub id: String,
    /// Its worktree (the task's workspace path).
    pub worktree: PathBuf,
    /// Its per-task Cargo target directory (the layout before slots).
    pub target: PathBuf,
    /// Finished or cancelled, checks not running, group clear: the
    /// task store's own `ended` test.
    pub ended: bool,
}

/// Reads the task store.
pub trait Facts: Send + Sync {
    /// Every task in the store.
    ///
    /// # Errors
    /// The store cannot be read; classes that need it are then skipped.
    fn tasks(&self) -> Result<Vec<TaskFact>, String>;
}

impl<F> Facts for F
where
    F: Fn() -> Result<Vec<TaskFact>, String> + Send + Sync,
{
    fn tasks(&self) -> Result<Vec<TaskFact>, String> {
        self()
    }
}

/// What one run reads the world through.
pub struct Env<'a> {
    pub layout: &'a Layout,
    /// `None` when no task store is known: classes 1 and 3 are skipped.
    pub facts: Option<&'a dyn Facts>,
    pub volumes: &'a dyn Volumes,
    pub processes: &'a dyn Processes,
    pub now: u64,
}

/// The tasks one check sees.
pub(crate) struct View {
    tasks: Option<Vec<TaskFact>>,
}

impl View {
    pub(crate) fn read(env: &Env<'_>) -> Self {
        Self {
            tasks: env.facts.and_then(|facts| facts.tasks().ok()),
        }
    }

    /// A task that is not ended and whose worktree or target is, holds,
    /// or lies inside `path`.
    fn live(&self, path: &Path) -> Option<&TaskFact> {
        self.tasks.as_ref()?.iter().find(|task| {
            !task.ended
                && [&task.worktree, &task.target]
                    .iter()
                    .any(|named| named.starts_with(path) || path.starts_with(named))
        })
    }

    fn by_target(&self, path: &Path) -> Option<&TaskFact> {
        self.tasks.as_ref()?.iter().find(|task| task.target == path)
    }

    fn by_worktree(&self, path: &Path) -> Option<&TaskFact> {
        self.tasks
            .as_ref()?
            .iter()
            .find(|task| task.worktree == path)
    }
}

/// Why a planned item qualifies.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_status: Option<String>,
    /// Seconds since last use.
    pub age_secs: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<String>,
    /// The locks taken (free), by file name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locks: Vec<String>,
    /// Processes using it: always zero for a planned item.
    pub processes: usize,
}

/// One folder a run deletes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub class: Class,
    pub path: PathBuf,
    pub bytes: u64,
    /// When it was last used (seconds since the epoch).
    pub touched: u64,
    /// Why it qualifies, in a few words.
    pub why: String,
    pub evidence: Evidence,
    /// The locks a deletion holds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locks: Vec<PathBuf>,
    /// What recreates a removed worktree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo: Option<Undo>,
}

/// A folder a class found but keeps, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    pub class: Class,
    pub path: PathBuf,
    pub why: String,
}

/// One volume's observation and its share of the plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumePlan {
    /// The first watched folder on it.
    pub root: PathBuf,
    pub space: Space,
    pub start: u64,
    pub stop: u64,
    pub emergency_below: u64,
    pub emergency: bool,
    /// What this run aims to free here: up to the stop level, at most the
    /// rule's per-run cap. Zero when the volume needs nothing.
    pub needed: u64,
    pub items: Vec<Item>,
}

impl VolumePlan {
    #[must_use]
    pub fn planned(&self) -> u64 {
        self.items.iter().map(|item| item.bytes).sum()
    }
}

/// What a run would do now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub volumes: Vec<VolumePlan>,
    pub kept: Vec<Kept>,
    /// Watched folders that are not a known cache, with their sizes:
    /// measured when the plan leaves a volume below its start level.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_cleaned: Vec<(PathBuf, u64)>,
}

impl Plan {
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.volumes.iter().flat_map(|volume| volume.items.iter())
    }
}

/// A folder some class names, before its checks.
#[derive(Clone, Debug)]
struct Candidate {
    path: PathBuf,
    touched: u64,
    why: String,
    task: Option<(String, &'static str)>,
    markers: Vec<String>,
    locks: Vec<PathBuf>,
}

impl Candidate {
    fn new(_class: Class, path: PathBuf, touched: u64, why: impl Into<String>) -> Self {
        Self {
            path,
            touched,
            why: why.into(),
            task: None,
            markers: Vec::new(),
            locks: Vec::new(),
        }
    }
}

/// The checks every candidate passes, while planning and again just before
/// it is deleted: allowed and not denied, no symbolic link, not another
/// volume, no live task, its locks free (and held, for a deletion), no
/// process inside, and for a worktree, nothing unsaved.
pub(crate) fn check(
    layout: &Layout,
    rule: &Rule,
    view: &View,
    snapshot: &Snapshot,
    class: Class,
    path: &Path,
    locks: &[PathBuf],
) -> Result<(Held, Option<Undo>, usize), String> {
    if let Some(why) = layout.refuse(rule, path) {
        return Err(why.into());
    }
    if let Some(task) = view.live(path) {
        return Err(format!("task {} is still running", short(&task.id)));
    }
    let held = Held::take(locks)?;
    let open = snapshot.inside(path);
    if open > 0 {
        return Err(format!(
            "in use: {open} open files or working folders inside"
        ));
    }
    let undo = if class == Class::Worktrees {
        Some(git::removable(path)?)
    } else {
        None
    };
    Ok((held, undo, open))
}

fn short(id: &str) -> &str {
    &id[..id.len().min(12)]
}

fn days(secs: u64) -> u64 {
    secs / 86_400
}

/// Cached sizes, so repeated runs do not walk unchanged folders.
#[derive(Default, Serialize, Deserialize)]
struct Sizes {
    #[serde(default)]
    entries: BTreeMap<PathBuf, Sized>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct Sized {
    bytes: u64,
    touched: u64,
    at: u64,
}

impl Sizes {
    fn load(layout: &Layout) -> Self {
        std::fs::read(layout.sizes())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save(&self, layout: &Layout) {
        if let Ok(bytes) = serde_json::to_vec(self) {
            let _ = crate::store::write_atomic(&layout.sizes(), &bytes);
        }
    }

    fn measure(&mut self, path: &Path, home: &Path, touched: u64, now: u64) -> Result<u64, String> {
        if let Some(cached) = self.entries.get(path)
            && cached.touched == touched
            && now.saturating_sub(cached.at) < 6 * 3600
        {
            return Ok(cached.bytes);
        }
        let measured = paths::measure(path, home).map_err(|error| error.to_string())?;
        if measured.foreign {
            return Err("holds another volume".into());
        }
        self.entries.insert(
            path.to_owned(),
            Sized {
                bytes: measured.bytes,
                touched,
                at: now,
            },
        );
        Ok(measured.bytes)
    }
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| real_dir(path))
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

fn slot_number(path: &Path) -> Option<usize> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    name.rsplit_once("-slot-")?.1.parse().ok()
}

/// Checkouts' build directories: `target/` inside a Git checkout that
/// Cargo marked with `CACHEDIR.TAG` and Git ignores.
fn checkout_targets(layout: &Layout, rule: &Rule) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for pattern in &rule.classes.checkouts {
        for top in glob(pattern, &layout.home) {
            let target = top.join("target");
            if top.join(".git").exists()
                && real_dir(&target)
                && target.join("CACHEDIR.TAG").is_file()
                && git::ignored(&top, &target)
            {
                found.push(target);
            }
        }
    }
    found
}

fn agent_targets(layout: &Layout, rule: &Rule) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = rule
        .classes
        .agent_targets
        .iter()
        .flat_map(|pattern| glob(pattern, &layout.home))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// Gate build directories: folders under the gate that carry
/// `CACHEDIR.TAG`, at most four levels down, never through a link or into
/// `.git`.
fn gate_builds(layout: &Layout) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![(layout.gate(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        for path in entries(&dir) {
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.join("CACHEDIR.TAG").is_file() {
                found.push(path);
            } else if depth < 3 {
                stack.push((path, depth + 1));
            }
        }
    }
    found.sort();
    found
}

/// Every candidate of `class`, before checks, least recently used first,
/// with what the class itself keeps.
fn candidates(
    env: &Env<'_>,
    rule: &Rule,
    view: &View,
    class: Class,
) -> (Vec<Candidate>, Vec<Kept>) {
    let layout = env.layout;
    let now = env.now;
    let mut found = Vec::new();
    let mut kept = Vec::new();
    let keep = |path: &Path, why: &str, kept: &mut Vec<Kept>| {
        kept.push(Kept {
            class,
            path: path.to_owned(),
            why: why.into(),
        });
    };
    match class {
        Class::EndedTargets => {
            for path in entries(&layout.targets()) {
                let touched = paths::touched(&path);
                if let Some(slot) = slot_number(&path) {
                    if slot >= SLOTS {
                        let mut candidate = Candidate::new(
                            class,
                            path.clone(),
                            touched,
                            "slot past the slot count",
                        );
                        candidate.locks = inuse::locks_of(&path);
                        found.push(candidate);
                    }
                    continue;
                }
                if view.tasks.is_none() {
                    keep(&path, "task store unreadable", &mut kept);
                    continue;
                }
                match view.by_target(&path) {
                    Some(task) if task.ended && view.live(&path).is_none() => {
                        let mut candidate =
                            Candidate::new(class, path.clone(), touched, "its task ended");
                        candidate.task = Some((task.id.clone(), "ended"));
                        candidate.locks = inuse::locks_of(&path);
                        found.push(candidate);
                    }
                    Some(_) => keep(&path, "its task is still running", &mut kept),
                    None => keep(&path, "no task record", &mut kept),
                }
            }
        }
        Class::StaleTargets => {
            let idle = rule.classes.idle_days * 86_400;
            let stale = |path: PathBuf,
                         limit: u64,
                         what: &str,
                         found: &mut Vec<Candidate>,
                         kept: &mut Vec<Kept>| {
                let touched = paths::touched(&path);
                let age = now.saturating_sub(touched);
                if age >= limit {
                    let mut candidate = Candidate::new(
                        class,
                        path.clone(),
                        touched,
                        format!("{what}, unused {} days", days(age)),
                    );
                    candidate.locks = inuse::locks_of(&path);
                    found.push(candidate);
                } else {
                    keep(&path, &format!("{what}, used {} days ago", days(age)), kept);
                }
            };
            for path in entries(&layout.targets()) {
                if slot_number(&path).is_some_and(|slot| slot < SLOTS) {
                    stale(path, idle, "idle slot", &mut found, &mut kept);
                }
            }
            if real_dir(&layout.coder_one_target()) {
                stale(
                    layout.coder_one_target(),
                    idle,
                    "Coder One build",
                    &mut found,
                    &mut kept,
                );
            }
            let mut agents: Vec<(u64, PathBuf)> = agent_targets(layout, rule)
                .into_iter()
                .map(|path| (paths::touched(&path), path))
                .collect();
            agents.sort_by(|a, b| b.0.cmp(&a.0));
            for (index, (_, path)) in agents.into_iter().enumerate() {
                if index < rule.classes.keep {
                    keep(&path, "one of the most recent agent builds kept", &mut kept);
                } else {
                    stale(path, idle, "agent build", &mut found, &mut kept);
                }
            }
            let mut found_checkouts = Vec::new();
            for path in checkout_targets(layout, rule) {
                stale(
                    path,
                    rule.classes.checkout_days * 86_400,
                    "checkout build",
                    &mut found_checkouts,
                    &mut kept,
                );
            }
            for mut candidate in found_checkouts {
                candidate.markers = vec!["CACHEDIR.TAG".into(), "git-ignored".into()];
                found.push(candidate);
            }
        }
        Class::Worktrees => {
            for path in entries(&layout.worktrees()) {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if name.starts_with('.') || name.contains(".spare-") {
                    continue;
                }
                if view.tasks.is_none() {
                    keep(&path, "task store unreadable", &mut kept);
                    continue;
                }
                let touched = paths::touched_worktree(&path);
                match view.by_worktree(&path) {
                    Some(_) if view.live(&path).is_some() => {
                        keep(&path, "its task is still running", &mut kept);
                    }
                    Some(task) => {
                        let mut candidate =
                            Candidate::new(class, path.clone(), touched, "its task ended");
                        candidate.task = Some((task.id.clone(), "ended"));
                        found.push(candidate);
                    }
                    None => {
                        let age = now.saturating_sub(touched);
                        if age >= rule.classes.orphan_worktree_days * 86_400 {
                            found.push(Candidate::new(
                                class,
                                path.clone(),
                                touched,
                                format!("no task record, unused {} days", days(age)),
                            ));
                        } else {
                            keep(&path, "no task record, used recently", &mut kept);
                        }
                    }
                }
            }
        }
        Class::GatePools => {
            for path in gate_builds(layout) {
                let touched = paths::touched(&path);
                let age = now.saturating_sub(touched);
                if age >= rule.classes.gate_idle_hours * 3600 {
                    let mut candidate = Candidate::new(
                        class,
                        path.clone(),
                        touched,
                        format!("gate build, unused {} hours", age / 3600),
                    );
                    candidate.markers = vec!["CACHEDIR.TAG".into()];
                    candidate.locks = inuse::locks_of(&path);
                    found.push(candidate);
                } else {
                    keep(&path, "gate build used in the last hour", &mut kept);
                }
            }
        }
        Class::Incremental => {
            let mut targets: Vec<PathBuf> = entries(&layout.targets())
                .into_iter()
                .filter(|path| slot_number(path).is_some())
                .collect();
            if real_dir(&layout.coder_one_target()) {
                targets.push(layout.coder_one_target());
            }
            targets.extend(agent_targets(layout, rule));
            targets.extend(checkout_targets(layout, rule));
            for target in targets {
                let touched = paths::touched(&target);
                for profile in ["debug", "release"] {
                    let path = target.join(profile).join("incremental");
                    if !real_dir(&path) || !real_dir(&target.join(profile)) {
                        continue;
                    }
                    let mut candidate = Candidate::new(
                        class,
                        path,
                        touched,
                        format!("{profile} incremental cache"),
                    );
                    let mut slot = target.as_os_str().to_owned();
                    slot.push(".lock");
                    candidate.locks = vec![
                        PathBuf::from(slot),
                        target.join(profile).join(".cargo-lock"),
                    ];
                    found.push(candidate);
                }
            }
        }
        Class::Trash => {
            for path in entries(&layout.trash()) {
                let touched = paths::touched_worktree(&path);
                found.push(Candidate::new(
                    class,
                    path,
                    touched,
                    "in the background trash",
                ));
            }
        }
    }
    found.sort_by(|a, b| a.touched.cmp(&b.touched).then_with(|| a.path.cmp(&b.path)));
    (found, kept)
}

/// The folders whose volumes the rule watches.
fn roots(layout: &Layout, rule: &Rule) -> Vec<PathBuf> {
    let mut roots = vec![layout.openagents.clone()];
    for pattern in rule
        .classes
        .agent_targets
        .iter()
        .chain(&rule.classes.checkouts)
    {
        if let Some(parent) = crate::rule::expand(pattern, &layout.home).parent() {
            roots.push(parent.to_owned());
        }
    }
    roots.retain(|root| root.exists());
    roots.dedup();
    roots
}

/// Observe each watched volume once, by device.
pub fn observe(env: &Env<'_>, rule: &Rule) -> Vec<VolumePlan> {
    let mut volumes: Vec<VolumePlan> = Vec::new();
    for root in roots(env.layout, rule) {
        let Ok(space) = env.volumes.space(&root) else {
            continue;
        };
        if volumes
            .iter()
            .any(|volume| volume.space.device == space.device)
        {
            continue;
        }
        volumes.push(volume_plan(rule, root, space, false));
    }
    volumes
}

fn volume_plan(rule: &Rule, root: PathBuf, space: Space, force: bool) -> VolumePlan {
    let start = rule.goal.start.of(space.total);
    let stop = rule.goal.stop.of(space.total);
    let emergency_below = rule.goal.emergency.of(space.total);
    let needed = if force || space.free < start {
        stop.saturating_sub(space.free).min(rule.goal.max_freed)
    } else {
        0
    };
    VolumePlan {
        root,
        space,
        start,
        stop,
        emergency_below,
        emergency: space.free < emergency_below,
        needed,
        items: Vec::new(),
    }
}

/// What a run of `rule` would delete now. `force` (a run someone asked
/// for) plans for every volume below its stop level, not only those below
/// the start level.
pub fn plan(env: &Env<'_>, rule: &Rule, force: bool) -> Plan {
    let view = View::read(env);
    let snapshot = env.processes.snapshot();
    let mut sizes = Sizes::load(env.layout);
    let mut volumes: Vec<VolumePlan> = observe(env, rule)
        .into_iter()
        .map(|volume| volume_plan(rule, volume.root, volume.space, force))
        .collect();
    let emergency = volumes.iter().any(|volume| volume.emergency);
    let mut kept = Vec::new();
    let mut planned: Vec<PathBuf> = Vec::new();
    let classes: Vec<Class> = rule
        .actions
        .iter()
        .flat_map(|action| action.classes())
        .collect();
    for class in classes {
        if class == Class::Trash && !emergency {
            continue;
        }
        if volumes
            .iter()
            .all(|volume| volume.planned() >= volume.needed)
        {
            break;
        }
        if (class == Class::EndedTargets || class == Class::Worktrees) && env.facts.is_none() {
            continue;
        }
        let (found, class_kept) = candidates(env, rule, &view, class);
        kept.extend(class_kept);
        for candidate in found {
            if planned.iter().any(|done| candidate.path.starts_with(done)) {
                continue;
            }
            let Ok(meta) = std::fs::symlink_metadata(&candidate.path) else {
                continue;
            };
            let Some(volume) = volumes
                .iter_mut()
                .find(|volume| volume.space.device == meta.dev())
            else {
                continue;
            };
            if volume.planned() >= volume.needed {
                continue;
            }
            if rule.classes.report_only.contains(&class) {
                kept.push(Kept {
                    class,
                    path: candidate.path,
                    why: "report only".into(),
                });
                continue;
            }
            let snapshot = match &snapshot {
                Ok(snapshot) => snapshot,
                Err(why) => {
                    kept.push(Kept {
                        class,
                        path: candidate.path,
                        why: format!("cannot tell what is in use: {why}"),
                    });
                    continue;
                }
            };
            let checked = check(
                env.layout,
                rule,
                &view,
                snapshot,
                class,
                &candidate.path,
                &candidate.locks,
            );
            let (held, undo, processes) = match checked {
                Ok(checked) => checked,
                Err(why) => {
                    kept.push(Kept {
                        class,
                        path: candidate.path,
                        why,
                    });
                    continue;
                }
            };
            drop(held);
            let bytes = match sizes.measure(
                &candidate.path,
                &env.layout.home,
                candidate.touched,
                env.now,
            ) {
                Ok(bytes) => bytes,
                Err(why) => {
                    kept.push(Kept {
                        class,
                        path: candidate.path,
                        why,
                    });
                    continue;
                }
            };
            let (task, task_status) = match candidate.task {
                Some((id, status)) => (Some(id), Some(status.to_owned())),
                None => (None, None),
            };
            planned.push(candidate.path.clone());
            volume.items.push(Item {
                class,
                path: candidate.path,
                bytes,
                touched: candidate.touched,
                why: candidate.why,
                evidence: Evidence {
                    task,
                    task_status,
                    age_secs: env.now.saturating_sub(candidate.touched),
                    markers: candidate.markers,
                    locks: candidate
                        .locks
                        .iter()
                        .filter(|lock| lock.exists())
                        .filter_map(|lock| lock.file_name())
                        .map(|name| name.to_string_lossy().into_owned())
                        .collect(),
                    processes,
                },
                locks: candidate.locks,
                undo,
            });
        }
    }
    sizes.save(env.layout);
    let mut plan = Plan {
        volumes,
        kept,
        not_cleaned: Vec::new(),
    };
    if plan
        .volumes
        .iter()
        .any(|volume| volume.space.free + volume.planned() < volume.start)
    {
        plan.not_cleaned = not_cleaned(env.layout, rule);
    }
    plan
}

/// The rule's report-only folders with their sizes, largest first.
pub(crate) fn not_cleaned(layout: &Layout, rule: &Rule) -> Vec<(PathBuf, u64)> {
    let mut found: Vec<(PathBuf, u64)> = rule
        .safety
        .report
        .iter()
        .map(|entry| crate::rule::expand(entry, &layout.home))
        .filter(|path| real_dir(path) && !layout.home.starts_with(path))
        .filter_map(|path| {
            let bytes = paths::measure_report(&path, &layout.home).ok()?.bytes;
            Some((path, bytes))
        })
        .collect();
    found.sort_by(|a, b| b.1.cmp(&a.1));
    found
}
