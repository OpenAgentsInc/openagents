//! Task sources: where a `query` step's work comes from, and what the
//! lookup is allowed to return.
//!
//! A `delegate` step needs work to hand over, and until now the only work
//! a run could reach was the list its caller had already built. That makes
//! a fan-out over six tasks easy and a fan-out over "the open backlog"
//! impossible, because nothing between the operator's sentence and the
//! delegation can turn one into the other.
//!
//! This module is that step's other half. A `query` step names a
//! **source**; a host resolves the name against its own registry and reads
//! the work from what it finds. The arrangement is the one
//! [`crate::questions`] already uses for a `decide` step's wording, for the
//! same reason: the program stays a description of the work, and the part
//! that is specific to one machine stays on that machine.
//!
//! # A source is named, never written into a step
//!
//! "Run `gh issue list`" is not a step kind. A program that carried a
//! command would be code, and [NIP-PRG](../../../nips/openagents/NIP-PRG.md)'s
//! one structural promise is that a program carries none — which is what
//! makes one safe to read from a stranger. So a step names a slug, a host
//! resolves the slug or refuses the step, and what the slug means is a file
//! in `sources/` that the operator controls.
//!
//! # An explicit list is a source like any other
//!
//! [`From::Request`] is the list the request carried, and it is reached
//! through the same [`Source::read`] every other source is. That is
//! deliberate rather than tidy. The first real burndown will run on work
//! chosen by inspection, and work chosen by inspection must exercise the
//! ordering, the bound, and the collision record that a queried list will
//! later depend on. A second code path for the easy case is a second code
//! path nobody tests.
//!
//! # The lookup carries what it knows about collisions
//!
//! Work items that touch the same file cannot run beside each other, and
//! work that declares it comes after other work cannot run beside that
//! either. Neither fact needs a model: they are in the list.
//!
//! [`Selection::of`] therefore does two things with them. A declared order
//! is **enforced** — an item naming work that is still in the same list is
//! dropped from this batch and recorded as dropped, because running the two
//! together is wrong by construction rather than by judgment. A shared path
//! is **recorded** — every path more than one selected item touches is
//! named in the trace and put in front of the decision that follows.
//!
//! The asymmetry is the measurement's.
//! [#9414](https://github.com/OpenAgentsInc/openagents/issues/9414) put
//! plans whose tasks genuinely collide to four doors and eleven of twelve
//! answers cleared the 0.7 bound on the local ones, `kev-8b` at 0.96 and
//! 0.97, with the wrong answers sitting above the right ones. A gate in
//! that state is not the place to put a fact that can be computed, so the
//! fact is computed and recorded whether or not the gate reads it.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::capability::is_slug;
use crate::delegate::{Bounds, Task};

/// The source body version this reads.
pub const SOURCE_VERSION: u32 = 1;

/// The work-list document version this reads.
pub const LIST_VERSION: u32 = 1;

/// The variable that moves the source directory.
pub const DIR_ENV: &str = "CODER_SOURCE_DIR";

/// The source a `query` step reads when it names none: the work the
/// request carried.
pub const REQUEST: &str = "request";

/// The wall bound a work item's task carries before a `delegate` step
/// states its own. The same five minutes [`Task::reading`] uses.
const TASK_MINUTES: u64 = 5;

/// Where a source's answer comes from.
///
/// Both variants are reads. Neither runs anything: a source that spawned a
/// process would need the trust and the subprocess bounds that
/// [#9427](https://github.com/OpenAgentsInc/openagents/issues/9427) is
/// about, and a lookup that executed a manifest's argv on the strength of
/// having read that manifest is the finding rather than the fix. An
/// operator who wants the open issues writes them to a file with one
/// command of their own, and the program names the file's source.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum From {
    /// The work the request carried, in the order it carried it.
    Request,
    /// A work list on disk, under the workspace.
    File {
        /// A relative path inside the workspace. Absolute paths and
        /// parent-directory steps are refused when the source is read, so
        /// a source definition names a place in the checkout rather than a
        /// place on the computer.
        path: String,
    },
}

/// The order a lookup puts its answer in.
///
/// A burndown that silently reorders is not reproducible, so a source
/// states which of these it means and the run records the order it
/// produced.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Order {
    /// The order the source answered in. For a file, the order the file
    /// lists.
    #[default]
    Given,
    /// Ascending by identifier, compared as text. A source whose
    /// identifiers are numbers and wants them in numeric order pads them
    /// or declares `given`.
    Id,
}

impl Order {
    /// The word a trace records this order under.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Order::Given => "given",
            Order::Id => "id",
        }
    }
}

/// What a lookup does with more work than the step's bound allows.
///
/// Both answers are deterministic and the trace says which one ran. The
/// choice is the program's, because it is a statement about the work
/// rather than about the machine: dropping fifteen of twenty-one issues
/// without saying so is the failure a burndown makes first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OnOverflow {
    /// Keep the first `max_results` in the recorded order and record the
    /// rest as dropped.
    #[default]
    Truncate,
    /// Refuse the step. The operator narrows the query.
    Refuse,
}

impl OnOverflow {
    /// The policy a word names, or `None` for a word this host has no
    /// policy for.
    #[must_use]
    pub fn named(word: &str) -> Option<Self> {
        match word {
            "truncate" => Some(OnOverflow::Truncate),
            "refuse" => Some(OnOverflow::Refuse),
            _ => None,
        }
    }

    /// The word a trace records this policy under.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            OnOverflow::Truncate => "truncate",
            OnOverflow::Refuse => "refuse",
        }
    }
}

/// One source: a name a program can use, and what answers it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Source {
    pub v: u32,
    /// The slug a `query` step names.
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: String,
    /// Where the answer comes from.
    pub from: From,
    /// The order the lookup puts the answer in.
    #[serde(default)]
    pub order: Order,
}

impl Source {
    /// The built-in source: the work the request carried.
    ///
    /// Built in rather than shipped as a file, because it is the one
    /// source whose meaning cannot be anything else, and a host with no
    /// `sources/` directory still runs a program that hands its own work
    /// in.
    #[must_use]
    pub fn request() -> Self {
        Source {
            v: SOURCE_VERSION,
            slug: REQUEST.to_string(),
            name: "The work the request carried".to_string(),
            summary: "Reads the task list the request handed in, in the order it handed it."
                .to_string(),
            from: From::Request,
            order: Order::Given,
        }
    }

    /// Reads a source from a local file.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the file is not a source this host
    /// reads: unreadable, unparseable, a `v` it does not know, a slug
    /// outside the grammar, or a path that leaves the workspace.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let source: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        source
            .validate()
            .map_err(|reason| format!("{}: {reason}", path.display()))?;
        Ok(source)
    }

    /// Whether this source is one this host reads from.
    ///
    /// # Errors
    ///
    /// Returns the first reason it is not.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != SOURCE_VERSION {
            return Err(format!(
                "body version is {}, this version reads {SOURCE_VERSION}",
                self.v
            ));
        }
        if !is_slug(&self.slug) {
            return Err(format!("slug {:?} is not a source slug", self.slug));
        }
        if let From::File { path } = &self.from {
            let candidate = Path::new(path);
            if candidate.is_absolute() {
                return Err(format!(
                    "{path:?} is an absolute path, and a source names a place in the workspace"
                ));
            }
            if candidate
                .components()
                .any(|part| part == std::path::Component::ParentDir)
            {
                return Err(format!(
                    "{path:?} leaves the workspace, which a source may not"
                ));
            }
        }
        Ok(())
    }

    /// Where this source's answer comes from, in the words the trace
    /// records beside it.
    #[must_use]
    pub fn resolved_from(&self) -> String {
        match &self.from {
            From::Request => "the request".to_string(),
            From::File { path } => format!("file {path}"),
        }
    }

    /// The work this source answers with, before anything is ordered or
    /// bounded.
    ///
    /// `workspace` is the directory a file source is read under, and
    /// `carried` is the work the request handed in.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the source could not be read.
    pub fn read(&self, workspace: &Path, carried: &[Task]) -> Result<Vec<Work>, String> {
        match &self.from {
            From::Request => Ok(carried
                .iter()
                .enumerate()
                .map(|(n, task)| Work::of(n, task))
                .collect()),
            From::File { path } => {
                let full = workspace.join(path);
                let text = std::fs::read_to_string(&full)
                    .map_err(|e| format!("{}: {e}", full.display()))?;
                read_list(&text).map_err(|reason| format!("{}: {reason}", full.display()))
            }
        }
    }
}

/// One work item: what a delegation is handed, and what the lookup knows
/// about how it sits beside the others.
#[derive(Clone, Debug)]
pub struct Work {
    /// What this item is called. The trace records the selected
    /// identifiers in order, so a run that fanned out to six of
    /// twenty-one can say which six.
    pub id: String,
    /// The paths this item touches. A path two selected items share is a
    /// collision, and the lookup records every one.
    pub touches: Vec<String>,
    /// The identifiers this item comes after. An item naming work that is
    /// still in the same list is dropped from this batch.
    pub after: Vec<String>,
    /// The task a delegation runs.
    pub task: Task,
}

impl Work {
    /// One item from a task the request carried.
    ///
    /// The identifier is positional, which is what the per-requirement
    /// acceptance questions already use, and the file a read-only task
    /// names is a path it touches.
    #[must_use]
    pub fn of(n: usize, task: &Task) -> Self {
        Work {
            id: format!("t{}", n + 1),
            touches: task.reads.iter().cloned().collect(),
            after: Vec::new(),
            task: task.clone(),
        }
    }

    /// Whether this item's paths say anything its `reads` did not.
    ///
    /// A read-only task about one file touches that file, and writing the
    /// same path twice under two names tells a reader nothing. So the
    /// paths are recorded only when they add one, which is also what keeps
    /// a six-files-six-tasks plan reading the way it did before a lookup
    /// could see a collision at all.
    #[must_use]
    fn paths_add_anything(&self) -> bool {
        match self.touches.as_slice() {
            [] => false,
            [only] => self.task.reads.as_deref() != Some(only.as_str()),
            _ => true,
        }
    }

    /// The item as a state object and a trace record spell it.
    ///
    /// What the item does not carry is left out rather than written as an
    /// empty list, so a list with no collisions and no declared order
    /// reads the way it did before either existed.
    #[must_use]
    pub fn value(&self) -> Value {
        let mut body = serde_json::Map::new();
        body.insert("id".to_string(), json!(self.id));
        body.insert("prompt".to_string(), json!(self.task.prompt));
        body.insert("reads".to_string(), json!(self.task.reads));
        body.insert("writes".to_string(), json!(self.task.writes));
        if let Some(expects) = &self.task.expected {
            body.insert("expects".to_string(), json!(expects));
        }
        if self.paths_add_anything() {
            body.insert("touches".to_string(), json!(self.touches));
        }
        if !self.after.is_empty() {
            body.insert("after".to_string(), json!(self.after));
        }
        Value::Object(body)
    }
}

/// One work item a source declared, as a work list spells it.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Declared {
    id: String,
    prompt: String,
    #[serde(default)]
    purpose: String,
    #[serde(default)]
    reads: Option<String>,
    #[serde(default)]
    touches: Vec<String>,
    #[serde(default)]
    after: Vec<String>,
    #[serde(default)]
    writes: bool,
    /// The answer the item expects back, the way a CoderBench task's
    /// `expects` entry states one. An item without it is judged
    /// `unverifiable`, never passed.
    #[serde(default, alias = "expected")]
    expects: Option<String>,
}

/// A work list, as a file source's document spells it.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct List {
    v: u32,
    #[serde(default)]
    work: Vec<Declared>,
}

impl Declared {
    /// The work item this describes.
    fn into_work(self) -> Result<Work, String> {
        if self.id.trim().is_empty() {
            return Err("a work item with no identifier cannot be recorded".to_string());
        }
        if self.prompt.trim().is_empty() {
            return Err(format!("work item {:?} asks nothing", self.id));
        }
        let mut touches = self.touches;
        if let Some(reads) = &self.reads
            && !touches.iter().any(|path| path == reads)
        {
            touches.push(reads.clone());
        }
        touches.sort();
        touches.dedup();
        let purpose = match self.purpose.trim().is_empty() {
            false => self.purpose,
            true => match &self.reads {
                Some(reads) => format!("Read {reads} and answer one question."),
                None => format!("Do work item {}.", self.id),
            },
        };
        Ok(Work {
            id: self.id,
            touches,
            after: self.after,
            task: Task {
                prompt: self.prompt,
                purpose,
                reads: self.reads,
                expected: self.expects,
                bounds: Bounds::minutes(TASK_MINUTES),
                isolation: crate::delegate::Isolation::Directory,
                writes: self.writes,
            },
        })
    }
}

/// Reads a work list, refusing one this host does not read.
fn read_list(text: &str) -> Result<Vec<Work>, String> {
    let list: List = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if list.v != LIST_VERSION {
        return Err(format!(
            "work list version is {}, this version reads {LIST_VERSION}",
            list.v
        ));
    }
    let work: Vec<Work> = list
        .work
        .into_iter()
        .map(Declared::into_work)
        .collect::<Result<_, _>>()?;
    let mut seen: Vec<&str> = Vec::new();
    for item in &work {
        if seen.contains(&item.id.as_str()) {
            return Err(format!(
                "identifier {:?} is used twice, and a run that fanned out to six of twenty-one has to name which six",
                item.id
            ));
        }
        seen.push(&item.id);
    }
    Ok(work)
}

/// One work item the lookup left out, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dropped {
    pub id: String,
    pub reason: String,
}

/// One path more than one selected item touches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collision {
    pub path: String,
    pub work: Vec<String>,
}

/// What the bound did to the answer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    /// The answer fit.
    #[default]
    None,
    /// It did not, and the lookup kept the first `max_results`.
    Truncated,
    /// It did not, and the lookup refused.
    Refused,
}

impl Overflow {
    /// The word a trace records this under.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Overflow::None => "none",
            Overflow::Truncated => "truncated",
            Overflow::Refused => "refused",
        }
    }
}

/// What one `query` step looked up: the work that runs, the order it is
/// in, what was left out, and what collides.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// The source the step named.
    pub source: String,
    /// Where the answer came from.
    pub resolved_from: String,
    /// The order the lookup applied.
    pub order: Order,
    /// How many items the source answered with.
    pub found: usize,
    /// The identifiers of everything the source answered with, in the
    /// order the lookup put them in.
    pub ordered: Vec<String>,
    /// The work that runs, in that order.
    pub work: Vec<Work>,
    /// What was left out, each with why.
    pub dropped: Vec<Dropped>,
    /// Every path more than one selected item touches.
    pub collisions: Vec<Collision>,
    /// What the bound did.
    pub overflow: Overflow,
}

impl Selection {
    /// Orders one source's answer, holds it to the step's bound, and
    /// records what that cost.
    ///
    /// Three passes, in this order, because the order is what makes the
    /// result reproducible:
    ///
    /// 1. **Order.** Whatever the source said, put in the order the source
    ///    declares, and the identifiers are recorded as
    ///    [`Selection::ordered`].
    /// 2. **Declared order is enforced.** An item naming work that is
    ///    still in this list is dropped, because the two cannot run at
    ///    once and nothing here schedules a second batch. The check reads
    ///    the whole list rather than the part already admitted, so it does
    ///    not depend on which of the two the ordering put first.
    /// 3. **The bound.** More items than `max` either truncates or
    ///    refuses, and [`Selection::overflow`] says which.
    ///
    /// Collisions are computed last, over what survived, and are recorded
    /// rather than enforced. A shared path is a reason to refuse a
    /// fan-out; which pairs genuinely collide is the question #9414
    /// measured the gate on, and a lookup that answered it would be
    /// answering it with a different, quieter model.
    #[must_use]
    pub fn of(source: &Source, found: Vec<Work>, max: usize, on_overflow: OnOverflow) -> Self {
        let mut ordered = found;
        if source.order == Order::Id {
            ordered.sort_by(|a, b| a.id.cmp(&b.id));
        }
        let present: Vec<String> = ordered.iter().map(|work| work.id.clone()).collect();
        let mut dropped = Vec::new();
        let mut kept: Vec<Work> = Vec::new();
        for work in ordered {
            let blocked: Vec<&String> = work
                .after
                .iter()
                .filter(|id| present.contains(id))
                .collect();
            if blocked.is_empty() {
                kept.push(work);
                continue;
            }
            let names: Vec<String> = blocked.iter().map(|id| (*id).clone()).collect();
            dropped.push(Dropped {
                id: work.id,
                reason: format!(
                    "comes after {}, which this lookup also found",
                    names.join(", ")
                ),
            });
        }
        let overflow = match (kept.len() > max, on_overflow) {
            (false, _) => Overflow::None,
            (true, OnOverflow::Truncate) => Overflow::Truncated,
            (true, OnOverflow::Refuse) => Overflow::Refused,
        };
        if overflow == Overflow::Truncated {
            for work in kept.split_off(max) {
                dropped.push(Dropped {
                    id: work.id,
                    reason: format!("over the step's max_results of {max}"),
                });
            }
        }
        let collisions = collisions(&kept);
        Selection {
            source: source.slug.clone(),
            resolved_from: source.resolved_from(),
            order: source.order,
            found: present.len(),
            ordered: present,
            work: kept,
            dropped,
            collisions,
            overflow,
        }
    }

    /// The tasks a `delegate` step hands over, in the recorded order.
    #[must_use]
    pub fn tasks(&self) -> Vec<Task> {
        self.work.iter().map(|work| work.task.clone()).collect()
    }

    /// How many items run.
    #[must_use]
    pub fn len(&self) -> usize {
        self.work.len()
    }

    /// Whether nothing runs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.work.is_empty()
    }

    /// The identifiers that run, in order.
    #[must_use]
    pub fn selected(&self) -> Vec<String> {
        self.work.iter().map(|work| work.id.clone()).collect()
    }

    /// What the lookup answered, in one line.
    #[must_use]
    pub fn output(&self) -> String {
        format!("{} of {} work items", self.work.len(), self.found)
    }

    /// The collisions as a state object and a trace record spell them.
    #[must_use]
    pub fn collisions_value(&self) -> Value {
        json!(
            self.collisions
                .iter()
                .map(|collision| json!({
                    "path": collision.path,
                    "work": collision.work,
                }))
                .collect::<Vec<_>>()
        )
    }

    /// What was left out, as the trace records it.
    #[must_use]
    pub fn dropped_value(&self) -> Value {
        json!(
            self.dropped
                .iter()
                .map(|dropped| json!({
                    "id": dropped.id,
                    "reason": dropped.reason,
                }))
                .collect::<Vec<_>>()
        )
    }
}

/// Every path more than one item touches, by path.
fn collisions(work: &[Work]) -> Vec<Collision> {
    let mut by_path: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for item in work {
        for path in &item.touches {
            by_path.entry(path).or_default().push(item.id.clone());
        }
    }
    by_path
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(path, ids)| Collision {
            path: path.to_string(),
            work: ids,
        })
        .collect()
}

/// One file a host would not read work from, and why.
#[derive(Clone, Debug)]
pub struct Refused {
    pub source: String,
    pub reason: String,
}

/// The sources a host has resolved, and the ones it refused.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    sources: BTreeMap<String, Source>,
    refused: Vec<Refused>,
}

impl Registry {
    /// Reads every source in one directory.
    ///
    /// # Errors
    ///
    /// Returns the underlying error when the directory cannot be read.
    pub fn read(dir: &Path) -> Result<Self, String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();
        let mut registry = Registry::default();
        for path in paths {
            match Source::load(&path) {
                Ok(source) => {
                    registry.sources.insert(source.slug.clone(), source);
                }
                Err(reason) => registry.refused.push(Refused {
                    source: path.display().to_string(),
                    reason,
                }),
            }
        }
        Ok(registry)
    }

    /// Reads each directory in turn. The first definition of a slug wins,
    /// so an operator's own directory overrides the repository's.
    #[must_use]
    pub fn open(dirs: &[PathBuf]) -> Self {
        let mut merged = Registry::default();
        for dir in dirs {
            let Ok(registry) = Registry::read(dir) else {
                continue;
            };
            for (slug, source) in registry.sources {
                merged.sources.entry(slug).or_insert(source);
            }
            merged.refused.extend(registry.refused);
        }
        merged
    }

    /// One source by slug.
    ///
    /// `request` answers even on a host with no `sources/` directory,
    /// because the work the request carried is a source whose meaning
    /// cannot be anything else. A file defining it wins, so an operator
    /// can still say what their own machine means by it.
    #[must_use]
    pub fn get(&self, slug: &str) -> Option<Source> {
        match self.sources.get(slug) {
            Some(source) => Some(source.clone()),
            None if slug == REQUEST => Some(Source::request()),
            None => None,
        }
    }

    /// The slugs this host resolved from files, in order.
    #[must_use]
    pub fn slugs(&self) -> Vec<String> {
        self.sources.keys().cloned().collect()
    }

    /// The files this host would not read work from, each with its
    /// reason.
    #[must_use]
    pub fn refused(&self) -> &[Refused] {
        &self.refused
    }
}

/// Where a host looks for sources, in order.
#[must_use]
pub fn search(repository: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = env::var_os(DIR_ENV).filter(|dir| !dir.is_empty()) {
        dirs.push(PathBuf::from(dir));
    }
    if let Some(root) = repository {
        dirs.push(root.join("sources"));
    }
    if let Some(home) = env::var_os("HOME").filter(|home| !home.is_empty()) {
        dirs.push(PathBuf::from(home).join(".openagents").join("sources"));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work(id: &str, touches: &[&str], after: &[&str]) -> Work {
        Work {
            id: id.to_string(),
            touches: touches.iter().map(|path| (*path).to_string()).collect(),
            after: after.iter().map(|id| (*id).to_string()).collect(),
            task: Task::reading(&format!("do {id}"), touches.first().unwrap_or(&"a.rs")),
        }
    }

    fn list(work: Vec<Work>, max: usize, on_overflow: OnOverflow) -> Selection {
        Selection::of(&Source::request(), work, max, on_overflow)
    }

    #[test]
    fn the_request_is_a_source_like_any_other() {
        let carried = [Task::reading("one", "a.rs"), Task::reading("two", "b.rs")];
        let found = Source::request()
            .read(Path::new("/"), &carried)
            .expect("the request always reads");

        assert_eq!(
            found
                .iter()
                .map(|work| work.id.as_str())
                .collect::<Vec<_>>(),
            ["t1", "t2"],
            "the identifiers are the ones the acceptance questions already use"
        );
        assert_eq!(
            found[0].touches,
            ["a.rs"],
            "a file it reads is a file it touches"
        );
        assert!(found[0].after.is_empty());
    }

    #[test]
    fn the_order_is_recorded_and_the_bound_is_applied() {
        let selection = list(
            vec![
                work("c", &["c.rs"], &[]),
                work("a", &["a.rs"], &[]),
                work("b", &["b.rs"], &[]),
            ],
            2,
            OnOverflow::Truncate,
        );

        assert_eq!(selection.ordered, ["c", "a", "b"], "given order is kept");
        assert_eq!(selection.selected(), ["c", "a"]);
        assert_eq!(selection.overflow, Overflow::Truncated);
        assert_eq!(selection.dropped.len(), 1);
        assert_eq!(selection.dropped[0].id, "b");
        assert!(selection.dropped[0].reason.contains("max_results of 2"));
    }

    #[test]
    fn a_source_ordering_by_identifier_sorts_before_it_bounds() {
        let mut source = Source::request();
        source.order = Order::Id;
        let selection = Selection::of(
            &source,
            vec![
                work("c", &["c.rs"], &[]),
                work("a", &["a.rs"], &[]),
                work("b", &["b.rs"], &[]),
            ],
            2,
            OnOverflow::Truncate,
        );

        assert_eq!(selection.ordered, ["a", "b", "c"]);
        assert_eq!(
            selection.selected(),
            ["a", "b"],
            "which two survive is the order's answer"
        );
    }

    #[test]
    fn a_lookup_that_overflows_can_refuse_instead() {
        let selection = list(
            vec![work("a", &["a.rs"], &[]), work("b", &["b.rs"], &[])],
            1,
            OnOverflow::Refuse,
        );

        assert_eq!(selection.overflow, Overflow::Refused);
        assert_eq!(
            selection.selected(),
            ["a", "b"],
            "a refusal records what it refused over, and the step stops"
        );
    }

    /// The order the list declares is enforced, and the drop is recorded.
    /// #9391 must land before #9401, and a fan-out that ran both at once
    /// would be wrong by construction rather than by judgment.
    #[test]
    fn work_that_comes_after_work_in_the_same_list_is_dropped() {
        let selection = list(
            vec![
                work("9391", &["crates/gym/src/digest.rs"], &[]),
                work("9401", &["crates/gym/src/gate.rs"], &["9391"]),
                work("9404", &["docs/programs.md"], &["9999"]),
            ],
            6,
            OnOverflow::Truncate,
        );

        assert_eq!(selection.selected(), ["9391", "9404"]);
        assert_eq!(selection.dropped.len(), 1);
        assert_eq!(selection.dropped[0].id, "9401");
        assert!(selection.dropped[0].reason.contains("9391"));
        assert_eq!(
            selection.overflow,
            Overflow::None,
            "the drop is the list's own order, not the bound"
        );
    }

    /// Work that comes after something this lookup did not find runs: the
    /// dependency has already landed.
    #[test]
    fn work_that_comes_after_nothing_in_the_list_runs() {
        let selection = list(
            vec![work("9401", &["a.rs"], &["9391"])],
            6,
            OnOverflow::Truncate,
        );
        assert_eq!(selection.selected(), ["9401"]);
        assert!(selection.dropped.is_empty());
    }

    #[test]
    fn a_path_two_items_touch_is_recorded() {
        let selection = list(
            vec![
                work("a", &["crates/gym/src/suite.rs", "docs/gym/README.md"], &[]),
                work("b", &["crates/gym/src/suite.rs"], &[]),
                work("c", &["crates/coder/src/turn.rs"], &[]),
            ],
            6,
            OnOverflow::Truncate,
        );

        assert_eq!(selection.collisions.len(), 1);
        assert_eq!(selection.collisions[0].path, "crates/gym/src/suite.rs");
        assert_eq!(selection.collisions[0].work, ["a", "b"]);
    }

    #[test]
    fn six_tasks_over_six_files_collide_over_nothing() {
        let selection = list(
            (1..=6)
                .map(|n| work(&format!("t{n}"), &[&format!("{n}.rs")], &[]))
                .collect(),
            12,
            OnOverflow::Refuse,
        );

        assert!(selection.collisions.is_empty());
        assert!(selection.dropped.is_empty());
        assert_eq!(selection.overflow, Overflow::None);
    }

    #[test]
    fn a_work_list_reads_back_the_paths_it_names() {
        let work = read_list(
            r#"{"v":1,"work":[
                {"id":"9391","prompt":"Fix the digest","reads":"crates/gym/src/digest.rs",
                 "touches":["crates/gym/src/gate.rs"],"writes":true},
                {"id":"9401","prompt":"Rewrite the rationale","after":["9391"]}]}"#,
        )
        .expect("the list reads");

        assert_eq!(work.len(), 2);
        assert_eq!(
            work[0].touches,
            ["crates/gym/src/digest.rs", "crates/gym/src/gate.rs"],
            "a file it reads is a file it touches, and the paths are sorted"
        );
        assert!(work[0].task.writes);
        assert_eq!(
            work[0].task.purpose,
            "Read crates/gym/src/digest.rs and answer one question."
        );
        assert_eq!(work[1].after, ["9391"]);
    }

    /// A work item states the answer it expects under `expects`, the way
    /// a CoderBench task does, and the item reads back with it; an item
    /// without one carries none, and is judged unverifiable downstream.
    #[test]
    fn a_work_item_states_what_it_expects() {
        let work = read_list(
            r#"{"v":1,"work":[
                {"id":"count","prompt":"How many steps","reads":"a.rs","expects":"5"},
                {"id":"older","prompt":"How many calls","expected":"2"},
                {"id":"open","prompt":"Describe the module"}]}"#,
        )
        .expect("the list reads");

        assert_eq!(work[0].task.expected.as_deref(), Some("5"));
        assert_eq!(
            work[1].task.expected.as_deref(),
            Some("2"),
            "the older spelling still reads"
        );
        assert_eq!(work[2].task.expected, None);

        assert_eq!(work[0].value()["expects"], json!("5"));
        assert_eq!(work[1].value()["expects"], json!("2"));
        assert!(
            work[2].value().get("expects").is_none(),
            "an item that states nothing does not read back as expecting an empty answer"
        );
    }

    #[test]
    fn a_work_list_this_host_does_not_read_is_refused() {
        for (body, expected) in [
            (r#"{"v":7,"work":[]}"#, "version is 7"),
            (r#"{"v":1,"work":[{"id":"a","prompt":""}]}"#, "asks nothing"),
            (
                r#"{"v":1,"work":[{"id":"a","prompt":"x"},{"id":"a","prompt":"y"}]}"#,
                "used twice",
            ),
        ] {
            let reason = read_list(body).expect_err("this list is refused");
            assert!(reason.contains(expected), "{reason}");
        }
    }

    #[test]
    fn a_source_that_leaves_the_workspace_is_refused() {
        for path in ["/etc/passwd", "../elsewhere/work.json"] {
            let source = Source {
                from: From::File {
                    path: path.to_string(),
                },
                ..Source::request()
            };
            assert!(
                source.validate().is_err(),
                "{path} is not a place in the workspace"
            );
        }
    }

    #[test]
    fn the_repositorys_sources_read() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sources");
        let registry = Registry::read(&dir).expect("the repository carries a sources dir");

        assert!(registry.refused().is_empty(), "{:?}", registry.refused());
        assert!(registry.slugs().contains(&"work-list".to_string()));
        assert!(
            registry.get(REQUEST).is_some(),
            "the built-in source answers whatever is on disk"
        );
        assert!(registry.get("no-such-source").is_none());
    }
}
