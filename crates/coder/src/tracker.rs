//! Tracker intake: the pinned snapshot a `query` step's tracker source
//! reads, and the read-only acquisition that writes one.
//!
//! A `delegate` step's work can come from a tracker — the issues a project
//! holds, in the order the project holds them — but a `query` step still
//! names a source and a source still runs nothing. The arrangement keeps
//! the split [`crate::source`] already makes:
//!
//! - A host-owned adapter reads the tracker and writes a **snapshot**: a
//!   versioned, self-digested document that pins the repository, the base
//!   revision, and every issue's identity, state, version marker, body
//!   digest, and native blocked-by dependencies.
//! - A `tracker` source reads that file under the workspace — the same
//!   kind of read a `file` source makes — and the lookup's ordering,
//!   bound, and collision record apply unchanged.
//!
//! # The snapshot is the selection record
//!
//! [`Snapshot`] is the pin [#9507](https://github.com/OpenAgentsInc/openagents/issues/9507)
//! asks for: what a dispatch would run, computed once and digested as a
//! whole. The answer to "did the work change between selection and
//! dispatch" is [`Snapshot::revalidate`], which compares a fresh snapshot
//! against the pinned one and names every drift rather than guessing.
//!
//! # What is trusted and what is not
//!
//! The task an issue becomes — its prompt, the paths it touches, whether
//! it writes, and the answer it is graded against — comes from a
//! [`TaskMap`], a file the host owns and the tracker cannot write. An
//! issue body is context: it is digested, excerpted, and quoted into a
//! prompt under a label that says it is data, and it never becomes a
//! command, a permission, or a grant. A tracker read grants no permission
//! to post a comment, close an issue, or merge anything; this module has
//! no methods that write to a tracker at all.
//!
//! # What is blocked is said
//!
//! A snapshot that cannot be trusted is refused whole: a version this
//! reader does not know, an identifier used twice, a fetch that stopped
//! short of the page bound. A snapshot that is trusted but whose items
//! cannot all run says so per item — the work list carries a `blocked`
//! reason and the lookup drops the item with that reason recorded. A
//! dependency on an issue that is still open and in the same selection
//! stays an `after` edge instead, which the lookup already enforces.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::source;

/// The snapshot document version this reads.
pub const SNAPSHOT_VERSION: u32 = 1;

/// The tracker kind this reads. A snapshot that names another tracker is
/// a different contract, not an older one.
pub const SNAPSHOT_KIND: &str = "github-project";

/// The host the GitHub adapter speaks to when a scope names none.
pub const GITHUB: &str = "github.com";

/// The bytes of one issue body a snapshot keeps as prompt context.
pub const EXCERPT_BYTES: usize = 4 * 1024;

/// The bytes a snapshot document itself may be before it is read.
pub const SNAPSHOT_BYTES: u64 = 4 * 1024 * 1024;

/// The task-map document version this reads.
pub const TASKS_VERSION: u32 = 1;

/// The bytes a task-map document itself may be before it is read.
pub const TASKS_BYTES: u64 = 1024 * 1024;

/// What a fetch and a read may spend.
///
/// Every bound is stated rather than assumed: a fetch that runs past the
/// page or item bound is refused rather than truncated, because a partial
/// answer that reads as complete is the failure this contract exists to
/// prevent.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The most pages one acquisition reads.
    pub pages: usize,
    /// The most issues one snapshot holds.
    pub items: usize,
    /// The most bytes of one issue body kept as prompt context.
    pub body_bytes: usize,
    /// The most bytes one fetch's output may be.
    pub output_bytes: usize,
    /// The deadline one acquisition runs under, across all its pages.
    pub wall: Duration,
}

impl Limits {
    /// The bounds a host gets when it states none.
    #[must_use]
    pub fn bounded() -> Self {
        Limits {
            pages: 20,
            items: 500,
            body_bytes: EXCERPT_BYTES,
            output_bytes: 1024 * 1024,
            wall: Duration::from_secs(60),
        }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Limits::bounded()
    }
}

/// What one snapshot covers: one repository's part of one project on one
/// host. A source that pins a scope refuses a snapshot that names another.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Scope {
    /// The tracker host, for example `github.com`.
    pub host: String,
    /// The owner the project and the repository belong to.
    pub owner: String,
    /// The repository the work lands in.
    pub repo: String,
    /// The project number the items come from.
    pub project: u64,
}

impl Scope {
    /// A scope on `github.com`.
    #[must_use]
    pub fn github(owner: &str, repo: &str, project: u64) -> Self {
        Scope {
            host: GITHUB.to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
            project,
        }
    }

    /// The repository as `owner/repo`, the form a dependency names it in.
    #[must_use]
    pub fn repository(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    /// Whether this scope is one the contract accepts.
    fn validate(&self) -> Result<(), String> {
        for (what, name) in [
            ("host", &self.host),
            ("owner", &self.owner),
            ("repo", &self.repo),
        ] {
            if name.is_empty()
                || name.len() > 100
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            {
                return Err(format!("scope {what} {name:?} is not a name"));
            }
        }
        if self.project == 0 {
            return Err("scope project is a number above zero".to_string());
        }
        Ok(())
    }
}

/// The revision the work is selected against. A dispatch whose base moved
/// since the snapshot was pinned is dispatching on stale input.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Base {
    /// The ref the adapter resolved, for example `main`.
    pub branch: String,
    /// The commit the ref named when the snapshot was taken.
    pub revision: String,
}

/// One native blocked-by edge, as the tracker reported it.
///
/// `repo` is absent for a dependency in the scoped repository. A
/// dependency in another repository is outside this snapshot's scope and
/// blocks the item it belongs to. `state` is `open` or `closed`; an empty
/// `state` is a dependency whose state the fetch did not pin, which is
/// unknown completeness rather than a satisfied dependency.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Dependency {
    /// The issue number within its repository.
    pub number: u64,
    /// The repository as `owner/repo`; absent means the scoped one.
    #[serde(default)]
    pub repo: Option<String>,
    /// `open`, `closed`, or empty when the fetch did not say.
    #[serde(default)]
    pub state: String,
    /// The dependency's version marker, when the fetch said one.
    #[serde(default)]
    pub updated: String,
}

impl Dependency {
    /// The repository this dependency lives in, `owner/repo`.
    fn repository<'a>(&'a self, scope: &'a Scope) -> String {
        self.repo.clone().unwrap_or_else(|| scope.repository())
    }

    /// Whether this dependency sits in the scoped repository.
    fn in_scope(&self, scope: &Scope) -> bool {
        match &self.repo {
            None => true,
            Some(repo) => *repo == scope.repository(),
        }
    }
}

/// What an issue becomes when it runs: the host's words, not the
/// tracker's.
///
/// A task definition is the half of a work item the host owns — the
/// prompt, the paths the work touches, whether it writes, and the answer
/// it is graded against. The tracker supplies the issue; the task map
/// supplies the work; an issue no task definition names is a blocked
/// item, not a guessed one.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TaskDef {
    /// What the executor is asked, verbatim. The issue's title, link, and
    /// a bounded body excerpt are appended under a marker that calls them
    /// untrusted context.
    pub prompt: String,
    /// What the task is for, in the words the trace shows beside it.
    #[serde(default)]
    pub purpose: Option<String>,
    /// The file the task reads, when it reads one.
    #[serde(default)]
    pub reads: Option<String>,
    /// The paths the task touches, for the lookup's collision record.
    #[serde(default)]
    pub touches: Vec<String>,
    /// Whether the task writes. A writing task runs in a checkout of its
    /// own under the delegate step's isolation bound, the same rule a
    /// file work list already follows.
    #[serde(default)]
    pub writes: bool,
    /// The answer the task is graded against. An item without one is
    /// unverifiable downstream, never passed.
    #[serde(default)]
    pub expects: Option<String>,
}

impl TaskDef {
    /// Whether this definition is one the contract accepts.
    fn validate(&self, number: u64) -> Result<(), String> {
        if self.prompt.trim().is_empty() {
            return Err(format!("the task for #{number} asks nothing"));
        }
        for path in self.touches.iter().chain(self.reads.iter()) {
            workspace_relative(path).map_err(|reason| {
                format!("the task for #{number} names {path:?}, which {reason}")
            })?;
        }
        Ok(())
    }
}

/// The task definitions a host owns, keyed by issue number.
///
/// This is the file the acquisition reads beside the tracker: what each
/// issue becomes when it runs. It is host data, which is the point — the
/// tracker cannot write it, a fetched issue body cannot supply it, and a
/// snapshot that was taken against one map digests differently under
/// another.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TaskMap {
    /// The document version.
    pub v: u32,
    /// The definitions, by issue number.
    #[serde(default)]
    pub tasks: BTreeMap<u64, TaskDef>,
}

impl TaskMap {
    /// No definitions: every issue a snapshot holds is blocked for the
    /// missing requirement, which is the honest answer to "what does this
    /// issue become" when nobody said.
    #[must_use]
    pub fn empty() -> Self {
        TaskMap {
            v: TASKS_VERSION,
            tasks: BTreeMap::new(),
        }
    }

    /// Reads a task map from a local file, bounded to [`TASKS_BYTES`].
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the file is not a task map this host
    /// reads: unreadable, past the bound, unparseable, a `v` it does not
    /// know, or a definition that asks nothing or names a path outside
    /// the workspace.
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = read_capped(path, TASKS_BYTES, "a task map")?;
        let map: Self =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        map.validate()
            .map_err(|reason| format!("{}: {reason}", path.display()))?;
        Ok(map)
    }

    /// Whether every definition is one the contract accepts.
    ///
    /// # Errors
    ///
    /// Returns the first reason one is not.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != TASKS_VERSION {
            return Err(format!(
                "task map version is {}, this version reads {TASKS_VERSION}",
                self.v
            ));
        }
        for (number, task) in &self.tasks {
            task.validate(*number)?;
        }
        Ok(())
    }
}

/// One issue, pinned: who it is, where it lives, what state it was in,
/// and everything needed to notice it changed.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Issue {
    /// The issue number within its repository.
    pub number: u64,
    /// The repository the issue lives in, `owner/repo`. An item from
    /// outside the scoped repository is pinned and blocked, never
    /// dropped silently.
    pub repo: String,
    /// The tracker's node identity for the issue — the pin a re-created
    /// issue with the same number fails.
    #[serde(default)]
    pub node: String,
    /// The issue's title, as fetched.
    pub title: String,
    /// `open` or `closed`. A closed issue is evidence for another item's
    /// dependencies; it is never work.
    pub state: String,
    /// The tracker's version marker for the issue — its `updatedAt`.
    #[serde(default)]
    pub updated: String,
    /// Where a reader finds the issue.
    #[serde(default)]
    pub url: String,
    /// The SHA-256 of the body as fetched. The body itself is not pinned
    /// — the digest is — and [`Snapshot::revalidate`] reads a changed
    /// digest as a changed input.
    pub body_digest: String,
    /// The bounded excerpt quoted into the item's prompt as untrusted
    /// context.
    #[serde(default)]
    pub excerpt: Option<String>,
    /// The native blocked-by edges the fetch reported.
    #[serde(default)]
    pub blocked_by: Vec<Dependency>,
    /// Whether the fetch saw the whole blocked-by list. An absent field
    /// reads as `false` — missing dependency completeness cannot admit —
    /// so a snapshot that means complete writes `true` explicitly, and
    /// an acquisition that hit the dependency page bound writes `false`.
    #[serde(default)]
    pub blocked_by_complete: bool,
    /// What this issue becomes when it runs — the host's task definition,
    /// resolved at acquisition so the snapshot carries what it digested.
    #[serde(default)]
    pub task: Option<TaskDef>,
}

/// A project item the snapshot records but does not carry as an issue:
/// a draft, a pull request, an item the fetch read and classified as
/// not an issue. It is named so the record of what the project held is
/// complete. An item the fetch could not classify — one whose content
/// the credential cannot read, or one carrying no type — is not skipped:
/// it refuses the page, because it cannot be told apart from an issue.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Skipped {
    /// The project item's node identity.
    pub id: String,
    /// Why it is not an issue this snapshot carries.
    pub reason: String,
}

/// The pinned selection: one scope, one base, and every issue the project
/// held at fetch time.
///
/// `fetched` is when the adapter took the snapshot — unix seconds —
/// and is the one field the digest does not cover, because two reads of
/// the same tracker state are the same selection whatever the clock said.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Snapshot {
    /// The document version.
    pub v: u32,
    /// The tracker kind; [`SNAPSHOT_KIND`] is the one this reads.
    pub kind: String,
    /// What the snapshot covers.
    pub scope: Scope,
    /// The revision the work was selected against.
    pub base: Base,
    /// When the adapter took it, in unix seconds. Informational, and
    /// outside the digest.
    #[serde(default)]
    pub fetched: String,
    /// The issues, in the order the project held them.
    #[serde(default)]
    pub issues: Vec<Issue>,
    /// The project items that were not issues, each with why.
    #[serde(default)]
    pub skipped: Vec<Skipped>,
}

impl Snapshot {
    /// Reads a snapshot from a local file, bounded to [`SNAPSHOT_BYTES`].
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the file is not a snapshot this host
    /// reads: unreadable, past the bound, unparseable, or refused by
    /// [`Snapshot::validate`].
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = read_capped(path, SNAPSHOT_BYTES, "a snapshot")?;
        let snapshot: Self =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        snapshot
            .validate()
            .map_err(|reason| format!("{}: {reason}", path.display()))?;
        Ok(snapshot)
    }

    /// Parses a snapshot document.
    ///
    /// # Errors
    ///
    /// Returns the first reason the document is not a snapshot this host
    /// reads.
    pub fn parse(text: &str) -> Result<Self, String> {
        let snapshot: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Whether this snapshot is one this host reads.
    ///
    /// The checks are the ones a wrong answer cannot survive: the version,
    /// the kind, the scope's shape, every issue's identity and version
    /// markers, an identifier used twice, and a dependency an issue
    /// declares on itself.
    ///
    /// # Errors
    ///
    /// Returns the first reason it is not.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != SNAPSHOT_VERSION {
            return Err(format!(
                "snapshot version is {}, this version reads {SNAPSHOT_VERSION}",
                self.v
            ));
        }
        if self.kind != SNAPSHOT_KIND {
            return Err(format!(
                "snapshot kind is {:?}, this version reads {SNAPSHOT_KIND:?}",
                self.kind
            ));
        }
        self.scope.validate()?;
        if self.base.branch.is_empty() || self.base.revision.is_empty() {
            return Err("a snapshot pins the base it was taken against".to_string());
        }
        let mut seen = BTreeSet::new();
        for issue in &self.issues {
            issue.validate()?;
            if !seen.insert(issue.number) {
                return Err(format!(
                    "issue #{} is pinned twice, and a run that selects six of twenty-one has to name which six",
                    issue.number
                ));
            }
        }
        Ok(())
    }

    /// Whether this snapshot names the scope `expected` pins — the
    /// answer a source that states a scope asks before it reads a word
    /// of work.
    ///
    /// # Errors
    ///
    /// Returns the field that differs.
    pub fn check_scope(&self, expected: &Scope) -> Result<(), String> {
        if self.scope == *expected {
            return Ok(());
        }
        Err(format!(
            "the snapshot covers {}/{} project {} on {}, and this source reads {}/{} project {} on {}",
            self.scope.owner,
            self.scope.repo,
            self.scope.project,
            self.scope.host,
            expected.owner,
            expected.repo,
            expected.project,
            expected.host
        ))
    }

    /// The selection record, digested as a whole.
    ///
    /// Two snapshots with the same scope, base, issues, and task
    /// definitions are the same selection whatever the clock said at
    /// fetch time, so `fetched` is the one field left out.
    #[must_use]
    pub fn digest(&self) -> String {
        atif::digest(&self.pinned())
    }

    /// The fields the digest covers.
    fn pinned(&self) -> Value {
        json!({
            "v": self.v,
            "kind": self.kind,
            "scope": self.scope,
            "base": self.base,
            "issues": self.issues,
            "skipped": self.skipped,
        })
    }

    /// The snapshot as a work-list v1 document: the existing structured
    /// contract, rendered deterministically.
    ///
    /// Every open issue becomes a work item in the order the project held
    /// it. A ready item carries the host's task definition, the issue's
    /// title and excerpt as untrusted context, and `after` edges for the
    /// blocked-by dependencies that live in the scoped repository. An
    /// item that cannot run carries a `blocked` reason instead — the same
    /// document, so the lookup records it with everything else it
    /// dropped.
    ///
    /// Closed issues are dependency evidence, not work, and are left out.
    ///
    /// # Errors
    ///
    /// Returns the reason the snapshot could not render — unreachable for
    /// a snapshot that validated, and stated anyway because a renderer
    /// that could not render is a bug rather than a partial answer.
    pub fn to_list(&self) -> Result<Value, String> {
        self.validate()?;
        let open: Vec<&Issue> = self
            .issues
            .iter()
            .filter(|issue| issue.state == "open")
            .collect();
        let present: BTreeSet<u64> = open.iter().map(|issue| issue.number).collect();
        let cyclic = cycle_members(&open, &self.scope);
        let mut work = Vec::new();
        for issue in open {
            let mut item = serde_json::Map::new();
            let id = issue.number.to_string();
            item.insert("id".to_string(), json!(id));
            let after: Vec<String> = issue
                .blocked_by
                .iter()
                .filter(|dep| dep.in_scope(&self.scope))
                .map(|dep| dep.number.to_string())
                .collect();
            if !after.is_empty() {
                item.insert("after".to_string(), json!(after));
            }
            match self.blocked_reason(issue, &present, &cyclic) {
                Some(reason) => {
                    item.insert(
                        "prompt".to_string(),
                        json!(format!(
                            "Tracked issue {}#{}: {}",
                            issue.repo, issue.number, issue.title
                        )),
                    );
                    item.insert("blocked".to_string(), json!(reason));
                }
                None => {
                    let task = issue.task.as_ref().expect("a ready item has a task");
                    item.insert("prompt".to_string(), json!(prompt(issue, task)));
                    item.insert(
                        "purpose".to_string(),
                        json!(task.purpose.clone().unwrap_or_else(|| {
                            format!("Work tracked issue {}#{}.", issue.repo, issue.number)
                        })),
                    );
                    if let Some(reads) = &task.reads {
                        item.insert("reads".to_string(), json!(reads));
                    }
                    if !task.touches.is_empty() {
                        item.insert("touches".to_string(), json!(task.touches));
                    }
                    if task.writes {
                        item.insert("writes".to_string(), json!(true));
                    }
                    if let Some(expects) = &task.expects {
                        item.insert("expects".to_string(), json!(expects));
                    }
                }
            }
            work.push(Value::Object(item));
        }
        Ok(json!({ "v": source::LIST_VERSION, "work": work }))
    }

    /// The work this snapshot answers with, in the order the project held
    /// it — blocked items included, carrying their reasons, so the
    /// lookup's dropped record says what was left out and why.
    ///
    /// The conversion renders the work-list v1 document and reads it
    /// back, so the document a host inspects and the list a step runs are
    /// the same contract rather than two renderings that could drift.
    ///
    /// # Errors
    ///
    /// Returns the reason the snapshot did not produce work — a document
    /// that no longer validates, or a rendering bug.
    pub fn work(&self) -> Result<Vec<source::Work>, String> {
        let list = self.to_list()?;
        source::read_list(&list.to_string())
    }

    /// Why this issue cannot run, or `None` when it can.
    ///
    /// The order is the order a reader wants the reasons in: scope first,
    /// because an item outside the repository is not this snapshot's to
    /// dispatch at all; then the missing requirement, because a host can
    /// act on it; then the dependency facts, least fixable last.
    fn blocked_reason(
        &self,
        issue: &Issue,
        present: &BTreeSet<u64>,
        cyclic: &BTreeSet<u64>,
    ) -> Option<String> {
        let scoped = self.scope.repository();
        if issue.repo != scoped {
            return Some(format!(
                "belongs to {}, outside the scoped {scoped}",
                issue.repo
            ));
        }
        if issue.task.is_none() {
            return Some("no task definition names it".to_string());
        }
        if !issue.blocked_by_complete {
            return Some(
                "its blocked-by list was cut at the fetch bound, so its completeness is unknown"
                    .to_string(),
            );
        }
        if cyclic.contains(&issue.number) {
            return Some(
                "sits on a dependency cycle, and nothing in a cycle runs first".to_string(),
            );
        }
        for dep in &issue.blocked_by {
            if !dep.in_scope(&self.scope) {
                return Some(format!(
                    "is blocked by {}#{}, a dependency outside the scoped repository",
                    dep.repository(&self.scope),
                    dep.number
                ));
            }
            match dep.state.as_str() {
                "closed" => {}
                "open" if !present.contains(&dep.number) => {
                    return Some(format!(
                        "is blocked by #{}, which is open and outside this selection",
                        dep.number
                    ));
                }
                "open" => {}
                _ => {
                    return Some(format!(
                        "depends on #{}, whose state this snapshot does not pin",
                        dep.number
                    ));
                }
            }
        }
        None
    }

    /// What changed between this pinned snapshot and a fresh one: every
    /// drift a dispatch must re-select for, named.
    ///
    /// An empty answer is the only green light. A closed issue, a moved
    /// base, a changed version marker, a re-created issue, a new
    /// dependency, a different task definition, and work that appeared
    /// after the selection was pinned are all named — the dispatcher
    /// decides what to do with the list, and the list never pretends the
    /// input held still.
    #[must_use]
    pub fn revalidate(&self, fresh: &Snapshot) -> Vec<Drift> {
        let mut drift = Vec::new();
        if self.scope != fresh.scope {
            drift.push(Drift::of(
                "snapshot",
                format!(
                    "the scope moved from {} to {}",
                    self.scope.repository(),
                    fresh.scope.repository()
                ),
            ));
        }
        if self.base != fresh.base {
            drift.push(Drift::of(
                "base",
                format!(
                    "the base moved from {} at {} to {} at {}",
                    self.base.branch, self.base.revision, fresh.base.branch, fresh.base.revision
                ),
            ));
        }
        let fresh_by_number: BTreeMap<u64, &Issue> = fresh
            .issues
            .iter()
            .map(|issue| (issue.number, issue))
            .collect();
        let pinned_by_number: BTreeMap<u64, &Issue> = self
            .issues
            .iter()
            .map(|issue| (issue.number, issue))
            .collect();
        for issue in &self.issues {
            let item = format!("#{}", issue.number);
            let Some(now) = fresh_by_number.get(&issue.number) else {
                drift.push(Drift::of(item, "is no longer in the project".to_string()));
                continue;
            };
            if issue.node != now.node {
                drift.push(Drift::of(
                    item.clone(),
                    "was re-created — the pinned node is not the fresh one".to_string(),
                ));
            }
            if issue.state != now.state {
                drift.push(Drift::of(
                    item.clone(),
                    format!("was {} at selection and is {} now", issue.state, now.state),
                ));
            }
            if issue.updated != now.updated {
                drift.push(Drift::of(
                    item.clone(),
                    "changed since the selection was pinned".to_string(),
                ));
            }
            if issue.body_digest != now.body_digest {
                drift.push(Drift::of(item.clone(), "its body changed".to_string()));
            }
            let mut before = issue.blocked_by.clone();
            let mut after = now.blocked_by.clone();
            before.sort();
            after.sort();
            if before != after {
                drift.push(Drift::of(
                    item.clone(),
                    "its blocked-by dependencies changed".to_string(),
                ));
            }
            if issue.task != now.task {
                drift.push(Drift::of(
                    item.clone(),
                    "its task definition changed".to_string(),
                ));
            }
        }
        for issue in &fresh.issues {
            if issue.state == "open" && !pinned_by_number.contains_key(&issue.number) {
                drift.push(Drift::of(
                    format!("#{}", issue.number),
                    "appeared after the selection was pinned".to_string(),
                ));
            }
        }
        drift
    }
}

/// One way a fresh snapshot disagrees with the pinned one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drift {
    /// What the drift is about — an issue number, `base`, or `snapshot`.
    pub item: String,
    /// What changed.
    pub reason: String,
}

impl Drift {
    fn of(item: impl Into<String>, reason: String) -> Self {
        Drift {
            item: item.into(),
            reason,
        }
    }
}

impl Issue {
    fn validate(&self) -> Result<(), String> {
        if self.number == 0 {
            return Err("issue numbers start at 1".to_string());
        }
        if !is_repository(&self.repo) {
            return Err(format!(
                "issue #{} lives in {:?}, which is not an owner/repo",
                self.number, self.repo
            ));
        }
        if self.node.is_empty() {
            return Err(format!("issue #{} pins no node identity", self.number));
        }
        if self.title.trim().is_empty() {
            return Err(format!("issue #{} has no title", self.number));
        }
        if !matches!(self.state.as_str(), "open" | "closed") {
            return Err(format!(
                "issue #{} is {:?}, and an issue is open or closed",
                self.number, self.state
            ));
        }
        if self.updated.is_empty() {
            return Err(format!("issue #{} pins no version marker", self.number));
        }
        if self.body_digest.len() != 64
            || !self
                .body_digest
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(format!(
                "issue #{} pins {:?} as its body digest, which is not a SHA-256",
                self.number, self.body_digest
            ));
        }
        if let Some(excerpt) = &self.excerpt
            && excerpt.len() > EXCERPT_BYTES + 64
        {
            return Err(format!(
                "issue #{} carries a {}-byte excerpt, past the {EXCERPT_BYTES}-byte bound",
                self.number,
                excerpt.len()
            ));
        }
        for dep in &self.blocked_by {
            if dep.number == self.number {
                return Err(format!("issue #{} depends on itself", self.number));
            }
            if dep.number == 0 {
                return Err(format!("issue #{} names a dependency #0", self.number));
            }
            if let Some(repo) = &dep.repo
                && !is_repository(repo)
            {
                return Err(format!(
                    "issue #{} depends on {repo}#{}, which is not an owner/repo",
                    self.number, dep.number
                ));
            }
            if !matches!(dep.state.as_str(), "" | "open" | "closed") {
                return Err(format!(
                    "issue #{} depends on #{} in state {:?}, which is not a state",
                    self.number, dep.number, dep.state
                ));
            }
        }
        if let Some(task) = &self.task {
            task.validate(self.number)?;
        }
        Ok(())
    }
}

/// Whether `repo` reads as `owner/repo`.
fn is_repository(repo: &str) -> bool {
    let Some((owner, name)) = repo.split_once('/') else {
        return false;
    };
    [owner, name].iter().all(|part| {
        !part.is_empty()
            && part.len() <= 100
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    })
}

/// Reads a document file, refusing the moment it passes `cap` bytes.
///
/// The bound applies while the bytes arrive: a file of any size is never
/// held in memory first and measured afterwards, which is the difference
/// between a cap and a report.
fn read_capped(path: &Path, cap: u64, what: &str) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() as u64 > cap {
        return Err(format!(
            "{}: {cap} bytes is the most {what} may be, and this one is past it",
            path.display()
        ));
    }
    Ok(bytes)
}

/// Whether `path` names a place inside the workspace — relative, and
/// never reaching above it.
fn workspace_relative(path: &str) -> Result<(), String> {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err("is an absolute path, and a task names a place in the workspace".to_string());
    }
    if candidate
        .components()
        .any(|part| part == std::path::Component::ParentDir)
    {
        return Err("leaves the workspace, which a task may not".to_string());
    }
    Ok(())
}

/// The prompt a ready item runs: the host's words first, then the
/// tracker's context under a label that calls it what it is.
fn prompt(issue: &Issue, task: &TaskDef) -> String {
    let mut prompt = task.prompt.clone();
    prompt.push_str("\n\n## Untrusted tracker context\n\n");
    prompt.push_str(&format!(
        "Issue {}#{}: {}",
        issue.repo, issue.number, issue.title
    ));
    if !issue.url.is_empty() {
        prompt.push_str(&format!("\n{}", issue.url));
    }
    if let Some(excerpt) = &issue.excerpt
        && !excerpt.is_empty()
    {
        prompt.push_str("\n\n");
        prompt.push_str(excerpt);
    }
    prompt
}

/// The open issues that sit on a dependency cycle, by number.
///
/// Only same-repository edges into issues this snapshot holds as open can
/// cycle, so the graph is those edges. A cycle's members all block: a fan
/// out that picked one end of a cycle would be choosing which of two
/// ordered items runs first, and the list, not the chooser, owns that
/// order.
fn cycle_members(open: &[&Issue], scope: &Scope) -> BTreeSet<u64> {
    let edges: BTreeMap<u64, Vec<u64>> = open
        .iter()
        .map(|issue| {
            let deps = issue
                .blocked_by
                .iter()
                .filter(|dep| dep.in_scope(scope) && dep.state == "open")
                .map(|dep| dep.number)
                .filter(|number| open.iter().any(|other| other.number == *number))
                .collect();
            (issue.number, deps)
        })
        .collect();
    let mut members = BTreeSet::new();
    let mut marks: BTreeMap<u64, u8> = BTreeMap::new();
    let mut stack: Vec<u64> = Vec::new();
    for issue in open {
        visit(issue.number, &edges, &mut marks, &mut stack, &mut members);
    }
    members
}

/// One depth-first walk: `marks` is 0 unseen, 1 on the stack, 2 done, and
/// an edge back onto the stack names a cycle.
fn visit(
    number: u64,
    edges: &BTreeMap<u64, Vec<u64>>,
    marks: &mut BTreeMap<u64, u8>,
    stack: &mut Vec<u64>,
    members: &mut BTreeSet<u64>,
) {
    match marks.get(&number) {
        Some(2) => return,
        Some(1) => {
            if let Some(at) = stack.iter().position(|on| *on == number) {
                members.extend(stack[at..].iter());
            }
            return;
        }
        _ => {}
    }
    marks.insert(number, 1);
    stack.push(number);
    for dep in edges.get(&number).cloned().unwrap_or_default() {
        visit(dep, edges, marks, stack, members);
    }
    stack.pop();
    marks.insert(number, 2);
}

/// The GitHub half: one GraphQL document, a strict page parser, the
/// assembly that turns pages into a snapshot, and the supervised `gh`
/// subprocess a host-approved adapter runs it through.
///
/// Every function here is a read. Nothing in this module posts, edits,
/// closes, labels, or merges — the query document is the only wire
/// contract, and it is a `query`, not a `mutation`.
pub mod github {
    use std::collections::BTreeSet;
    use std::path::Path;
    use std::process::Command;
    use std::time::{Duration, Instant};

    use serde_json::Value;

    use super::{Base, Dependency, Issue, Limits, Scope, Skipped, Snapshot, TaskMap};
    use crate::capability;

    /// The items one page asks for.
    pub const PAGE_ITEMS: u32 = 50;

    /// The blocked-by edges one issue asks for. A longer list is a
    /// `blocked_by_complete: false`, never a silent cut.
    pub const PAGE_DEPENDENCIES: u32 = 20;

    /// The document one fetch sends. It asks for the project's items in
    /// order, each item's issue with its native blocked-by edges, and the
    /// scoped repository's default-branch head — the base revision the
    /// snapshot pins — in the same round trip.
    pub const QUERY: &str = r#"query($owner: String!, $repo: String!, $project: Int!, $cursor: String) {
  organization(login: $owner) {
    projectV2(number: $project) {
      title
      items(first: 50, after: $cursor) {
        pageInfo { hasNextPage endCursor }
        nodes {
          id
          content {
            __typename
            ... on Issue {
              id
              number
              title
              state
              updatedAt
              url
              body
              repository { nameWithOwner }
              blockedBy(first: 20) {
                pageInfo { hasNextPage }
                nodes {
                  number
                  state
                  updatedAt
                  repository { nameWithOwner }
                }
              }
            }
          }
        }
      }
    }
  }
  repository(owner: $owner, name: $repo) {
    defaultBranchRef {
      name
      target { oid }
    }
  }
}"#;

    /// The argv one page runs through, with the adapter's resolved path
    /// in front. `gh api graphql` puts every field under `variables`
    /// except the query itself; `project` is `-F` so it goes as an
    /// integer, and `cursor` is absent rather than empty on the first
    /// page.
    ///
    /// `--hostname` comes from the captured scope, not from ambient
    /// `GH_HOST`: every page speaks to the host the snapshot claims, so
    /// the scope cannot drift mid-acquisition.
    #[must_use]
    pub fn argv(scope: &Scope, cursor: Option<&str>) -> Vec<String> {
        let mut argv = vec![
            "api".to_string(),
            "graphql".to_string(),
            "--hostname".to_string(),
            scope.host.clone(),
            "-f".to_string(),
            format!("query={QUERY}"),
            "-f".to_string(),
            format!("owner={}", scope.owner),
            "-f".to_string(),
            format!("repo={}", scope.repo),
            "-F".to_string(),
            format!("project={}", scope.project),
        ];
        if let Some(cursor) = cursor {
            argv.push("-f".to_string());
            argv.push(format!("cursor={cursor}"));
        }
        argv
    }

    /// One issue as a page reported it, before the task map and the
    /// digests land.
    #[derive(Clone, Debug)]
    pub struct Fetched {
        pub number: u64,
        pub repo: String,
        pub node: String,
        pub title: String,
        pub state: String,
        pub updated: String,
        pub url: String,
        pub body: String,
        pub blocked_by: Vec<Dependency>,
        pub blocked_by_complete: bool,
    }

    /// One page, parsed: the issues it carried, the items it skipped, the
    /// base it saw, and the cursor that continues it.
    #[derive(Clone, Debug, Default)]
    pub struct Page {
        /// Every project item identity the page carried, issues and
        /// skipped alike — the acquisition deduplicates on them, because
        /// an item reported twice means the pages overlap.
        pub ids: Vec<String>,
        pub issues: Vec<Fetched>,
        pub skipped: Vec<Skipped>,
        pub base: Option<Base>,
        /// The cursor the next page reads, when the project has more.
        pub next: Option<String>,
    }

    /// Reads one GraphQL response body.
    ///
    /// The parser is strict where leniency would lie: a response carrying
    /// `errors` is refused with the messages it carried, a missing
    /// organization or project is refused rather than read as empty, an
    /// issue missing a field the pin needs is refused rather than
    /// defaulted, and a `blockedBy` page that says it has more marks the
    /// issue's dependency list incomplete rather than silently short.
    /// An item the fetch cannot classify — one with no identity, one
    /// whose content the credential cannot read, or one carrying no
    /// type — is an incomplete page, not a skipped item: it cannot be
    /// told apart from an issue the snapshot should carry.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the body is not a page this host
    /// reads.
    pub fn page(body: &str) -> Result<Page, String> {
        let value: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
        if let Some(errors) = value.get("errors").and_then(Value::as_array)
            && !errors.is_empty()
        {
            let messages: Vec<String> = errors
                .iter()
                .map(|error| {
                    error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("an error with no message")
                        .to_string()
                })
                .collect();
            return Err(format!(
                "the tracker answered with errors: {}",
                messages.join("; ")
            ));
        }
        let data = value
            .get("data")
            .filter(|data| data.is_object())
            .ok_or_else(|| "the answer carries no data object".to_string())?;
        let base = parse_base(data)?;
        let organization = data.get("organization").filter(|o| o.is_object()).ok_or_else(|| {
            "the answer names no organization — the scope's owner or a credential's reach is wrong"
                .to_string()
        })?;
        let project = organization
            .get("projectV2")
            .filter(|p| p.is_object())
            .ok_or_else(|| "the organization has no project of that number".to_string())?;
        let items = project
            .get("items")
            .filter(|i| i.is_object())
            .ok_or_else(|| "the project carries no items object".to_string())?;
        let info = items
            .get("pageInfo")
            .filter(|i| i.is_object())
            .ok_or_else(|| "the items page carries no pageInfo".to_string())?;
        let next = match info.get("hasNextPage").and_then(Value::as_bool) {
            Some(true) => Some(
                info.get("endCursor")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "a continued page names no cursor".to_string())?
                    .to_string(),
            ),
            Some(false) => None,
            None => return Err("pageInfo does not say whether another page exists".to_string()),
        };
        let nodes = items
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| "the items page carries no nodes".to_string())?;
        let mut page = Page {
            base,
            next,
            ..Page::default()
        };
        for node in nodes {
            let id = node
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or_else(|| {
                    "a project item carries no identity, and an item the fetch cannot name is one it cannot account for".to_string()
                })?
                .to_string();
            page.ids.push(id.clone());
            let Some(content) = node.get("content").filter(|c| c.is_object()) else {
                return Err(format!(
                    "project item {id} answered no content — an item the credential cannot read is an incomplete page, not a skipped one"
                ));
            };
            match content.get("__typename").and_then(Value::as_str) {
                Some("Issue") => page.issues.push(parse_issue(content)?),
                Some(other) => page.skipped.push(Skipped {
                    id,
                    reason: format!("a {other} project item, not an issue"),
                }),
                None => {
                    return Err(format!(
                        "project item {id} carries no type, and an item the fetch cannot classify is not one it can skip"
                    ));
                }
            }
        }
        Ok(page)
    }

    /// The base revision a page reports — the scoped repository's
    /// default-branch head in the same round trip.
    fn parse_base(data: &Value) -> Result<Option<Base>, String> {
        let Some(repository) = data.get("repository") else {
            return Ok(None);
        };
        let repository = repository
            .as_object()
            .ok_or_else(|| "the scoped repository did not answer".to_string())?;
        let branch = repository
            .get("defaultBranchRef")
            .filter(|r| r.is_object())
            .ok_or_else(|| "the scoped repository names no default branch".to_string())?;
        let name = branch
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "the default branch carries no name".to_string())?;
        let revision = branch
            .get("target")
            .and_then(|target| target.get("oid"))
            .and_then(Value::as_str)
            .ok_or_else(|| "the default branch names no commit".to_string())?;
        Ok(Some(Base {
            branch: name.to_string(),
            revision: revision.to_string(),
        }))
    }

    /// One issue node, strict: a missing field is a refused page, because
    /// a snapshot that cannot pin what it read is worse than no snapshot.
    fn parse_issue(content: &Value) -> Result<Fetched, String> {
        let want = |field: &str| -> Result<&Value, String> {
            content
                .get(field)
                .filter(|value| !value.is_null())
                .ok_or_else(|| format!("an issue node carries no {field}"))
        };
        let text = |field: &str| -> Result<String, String> {
            want(field).and_then(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("an issue's {field} is not text"))
            })
        };
        let number = want("number")?
            .as_u64()
            .ok_or_else(|| "an issue's number is not a number".to_string())?;
        let state = match text("state")?.as_str() {
            "OPEN" => "open",
            "CLOSED" => "closed",
            other => {
                return Err(format!(
                    "an issue is {other:?}, and an issue is OPEN or CLOSED"
                ));
            }
        };
        let repo = want("repository")?
            .get("nameWithOwner")
            .and_then(Value::as_str)
            .ok_or_else(|| "an issue names no repository".to_string())?
            .to_string();
        let mut blocked_by = Vec::new();
        let mut blocked_by_complete = true;
        match content.get("blockedBy") {
            Some(connection) if connection.is_object() => {
                if connection
                    .get("pageInfo")
                    .and_then(|info| info.get("hasNextPage"))
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
                {
                    blocked_by_complete = false;
                }
                for dep in connection
                    .get("nodes")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                {
                    blocked_by.push(parse_dependency(&dep)?);
                }
            }
            // A schema with no `blockedBy` answered nothing about the
            // issue's dependencies, which is unknown completeness.
            _ => blocked_by_complete = false,
        }
        Ok(Fetched {
            number,
            repo,
            node: text("id")?,
            title: text("title")?,
            state: state.to_string(),
            updated: text("updatedAt")?,
            url: content
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            body: content
                .get("body")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            blocked_by,
            blocked_by_complete,
        })
    }

    /// One blocked-by edge. A dependency that cannot be identified is a
    /// refused page; one whose state is absent parses with an empty
    /// `state`, which the item's readiness reads as unpinned.
    fn parse_dependency(node: &Value) -> Result<Dependency, String> {
        let number = node
            .get("number")
            .and_then(Value::as_u64)
            .ok_or_else(|| "a dependency carries no number".to_string())?;
        let state = match node.get("state").and_then(Value::as_str) {
            Some("OPEN") => "open".to_string(),
            Some("CLOSED") => "closed".to_string(),
            _ => String::new(),
        };
        Ok(Dependency {
            number,
            repo: node
                .get("repository")
                .and_then(|repo| repo.get("nameWithOwner"))
                .and_then(Value::as_str)
                .map(str::to_string),
            state,
            updated: node
                .get("updatedAt")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        })
    }

    /// Turns the fetched pages into a pinned snapshot.
    ///
    /// Assembly refuses what a snapshot cannot honestly carry: an issue
    /// reported twice, more items than the bound, a base the pages
    /// disagreed about. Task definitions resolve here — against the
    /// host's map, so the snapshot digests the task it will dispatch —
    /// and each issue's body becomes a digest plus a bounded excerpt.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why these pages are not a snapshot.
    pub fn assemble(
        scope: Scope,
        fetched: Vec<Fetched>,
        skipped: Vec<Skipped>,
        base: Base,
        tasks: &TaskMap,
        limits: &Limits,
        fetched_at: &str,
    ) -> Result<Snapshot, String> {
        if fetched.len() + skipped.len() > limits.items {
            return Err(format!(
                "the project holds {} items against a bound of {}",
                fetched.len() + skipped.len(),
                limits.items
            ));
        }
        let mut seen = BTreeSet::new();
        for issue in &fetched {
            if !seen.insert(issue.number) {
                return Err(format!(
                    "the tracker reported issue #{} twice",
                    issue.number
                ));
            }
        }
        let snapshot = Snapshot {
            v: super::SNAPSHOT_VERSION,
            kind: super::SNAPSHOT_KIND.to_string(),
            scope,
            base,
            fetched: fetched_at.to_string(),
            issues: fetched
                .into_iter()
                .map(|issue| Issue {
                    number: issue.number,
                    repo: issue.repo,
                    node: issue.node,
                    title: issue.title,
                    state: issue.state,
                    updated: issue.updated,
                    url: issue.url,
                    body_digest: capability::digest_bytes(issue.body.as_bytes()),
                    excerpt: excerpt(&issue.body, limits.body_bytes),
                    blocked_by: issue.blocked_by,
                    blocked_by_complete: issue.blocked_by_complete,
                    task: tasks.tasks.get(&issue.number).cloned(),
                })
                .collect(),
            skipped,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// The bounded excerpt a prompt quotes: the body up to the byte cap,
    /// cut on a character boundary and marked when it was cut.
    fn excerpt(body: &str, max: usize) -> Option<String> {
        if body.is_empty() {
            return None;
        }
        if body.len() <= max {
            return Some(body.to_string());
        }
        let mut cut = max;
        while !body.is_char_boundary(cut) {
            cut -= 1;
        }
        Some(format!("{}\n…", &body[..cut]))
    }

    /// The page loop one acquisition runs.
    ///
    /// `fetch` answers one page by cursor under the wall that remains.
    /// The guards are the difference between a snapshot and a partial
    /// answer that reads as complete: a cursor the tracker already gave
    /// is a loop rather than progress, an item identity reported twice
    /// means the pages overlap, and skipped items count toward the item
    /// bound because they are items the project holds.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the pages are not a complete read:
    /// the page or item bound passed, the deadline passed, a cursor or an
    /// item identity repeated, the base moved mid-read, or a fetch
    /// failed.
    pub fn paginate(
        limits: &Limits,
        mut fetch: impl FnMut(Option<&str>, Duration) -> Result<Page, String>,
    ) -> Result<(Vec<Fetched>, Vec<Skipped>, Base), String> {
        let started = Instant::now();
        let mut fetched = Vec::new();
        let mut skipped = Vec::new();
        let mut base: Option<Base> = None;
        let mut cursor: Option<String> = None;
        let mut cursors = BTreeSet::new();
        let mut ids = BTreeSet::new();
        let mut pages = 0;
        loop {
            pages += 1;
            if pages > limits.pages {
                return Err(format!(
                    "the project has more than {} pages of items, and a fetch that stops short is a partial answer",
                    limits.pages
                ));
            }
            let remaining = limits
                .wall
                .checked_sub(started.elapsed())
                .filter(|left| !left.is_zero())
                .ok_or_else(|| "the acquisition's deadline passed mid-fetch".to_string())?;
            let parsed = fetch(cursor.as_deref(), remaining)?;
            if let Some(seen) = &parsed.base {
                match &base {
                    None => base = Some(seen.clone()),
                    Some(pinned) if pinned != seen => {
                        return Err(
                            "the base moved while the pages were being read — the snapshot would be two selections".to_string(),
                        );
                    }
                    _ => {}
                }
            }
            for id in parsed.ids {
                if !ids.insert(id.clone()) {
                    return Err(format!(
                        "the tracker reported item {id} twice — a page that repeats is a read this snapshot cannot trust"
                    ));
                }
            }
            fetched.extend(parsed.issues);
            skipped.extend(parsed.skipped);
            if fetched.len() + skipped.len() > limits.items {
                return Err(format!(
                    "the project holds more than {} items, and a fetch that stops short is a partial answer",
                    limits.items
                ));
            }
            match parsed.next {
                Some(next) if !cursors.insert(next.clone()) => {
                    return Err(format!(
                        "the tracker returned cursor {next:?} twice — repeating a page is not progress"
                    ));
                }
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        let base = base.ok_or_else(|| "no page named the scoped repository's base".to_string())?;
        Ok((fetched, skipped, base))
    }

    /// Acquires a snapshot through a supervised `gh` subprocess the
    /// operator approved.
    ///
    /// This is the live half of the intake, and it borrows the trust
    /// contract rather than inventing one: `entry` is a capability
    /// manifest whose `detect.binary` names `gh`, and the fetch runs only
    /// when [`capability::Trust`] holds a record approving that manifest —
    /// the same approval a probe needs, re-decided against the store as
    /// it stands now. The approved adapter's pinned path is what runs;
    /// under an unconditional trust the manifest's binary resolves fresh.
    /// Either way the resolved file must be named `gh` — the argv is
    /// `gh api graphql`, and an approval that pins anything else is not
    /// this adapter whatever the manifest's word said.
    ///
    /// The scope is validated and captured before the first page runs,
    /// and every page's argv is built from that one capture — host
    /// included, as `--hostname` — so an ambient `GH_HOST` or a mutated
    /// scope cannot move a later page to a different project.
    ///
    /// `env` is the caller's environment for the child — `GH_TOKEN` or
    /// `GH_HOST`, whatever the host supplies. Its values are secrets:
    /// they are handed to the child, never stored, and scrubbed from any
    /// error this returns. The snapshot carries no credential, and no log
    /// line in this module prints one.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why no snapshot exists: the scope is not
    /// a scope, the manifest is unapproved, the adapter is not a
    /// subprocess transport, the binary resolves to nothing or is not
    /// `gh`, a page refused, the deadline passed, or the assembled pages
    /// are not a snapshot.
    pub fn acquire(
        entry: &capability::Entry,
        trust: &capability::Trust,
        workspace: &Path,
        scope: &Scope,
        limits: &Limits,
        tasks: &TaskMap,
        env: &[(String, String)],
    ) -> Result<Snapshot, String> {
        scope.validate()?;
        if entry.manifest.transport != capability::SUBPROCESS {
            return Err(format!(
                "transport {} is not an argv this host runs",
                entry.manifest.transport
            ));
        }
        let adapter = match trust.decide_verified(entry, workspace) {
            capability::Verified::Approved(record) => record.adapter,
            capability::Verified::Unconditional => {
                capability::resolve(&entry.manifest.detect.binary, &capability::search_dirs())
                    .ok_or_else(|| {
                        format!(
                            "{} resolves to nothing on this host",
                            entry.manifest.detect.binary
                        )
                    })?
            }
            capability::Verified::Unapproved(why) => return Err(why),
        };
        if adapter.file_name() != Some(std::ffi::OsStr::new("gh")) {
            return Err(format!(
                "the approved adapter {} is not `gh`, and this acquisition only drives the GitHub CLI",
                adapter.display()
            ));
        }
        let secrets: Vec<String> = env
            .iter()
            .map(|(_, value)| value.clone())
            .filter(|value| value.len() >= 8)
            .collect();
        let fetch = Fetch {
            adapter: &adapter,
            workspace,
            scope,
            limits,
            env,
            secrets: &secrets,
        };
        let (fetched, skipped, base) = paginate(limits, |cursor, remaining| {
            fetch.page(cursor, remaining).and_then(|body| page(&body))
        })?;
        let fetched_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs().to_string())
            .unwrap_or_default();
        assemble(
            scope.clone(),
            fetched,
            skipped,
            base,
            tasks,
            limits,
            &fetched_at,
        )
    }

    /// One acquisition's running context: the approved adapter, the
    /// directory it runs in, the scope it was captured for, and the
    /// bounds and credentials the caller set. Bundled because a page run
    /// needs all of them.
    struct Fetch<'a> {
        adapter: &'a Path,
        workspace: &'a Path,
        scope: &'a Scope,
        limits: &'a Limits,
        env: &'a [(String, String)],
        secrets: &'a [String],
    }

    impl Fetch<'_> {
        /// Runs one page's argv through the bounded supervisor: the child
        /// in a process group of its own, the deadline terminating the
        /// group and the direct child reaped, and both output streams
        /// drained into capped buffers as they arrive. Nothing is written
        /// to a file — there is no path for a name to guess and nothing
        /// to leave behind.
        ///
        /// The argv is built here, per page, from the one scope the
        /// acquisition captured, so a later page cannot be asked for a
        /// different host, owner, repository, or project. A page cut at
        /// the output cap is a refused page: half a GraphQL body is a
        /// partial answer, not a short one.
        fn page(&self, cursor: Option<&str>, wall: Duration) -> Result<String, String> {
            let mut command = Command::new(self.adapter);
            command
                .args(argv(self.scope, cursor))
                .envs(self.env.iter().cloned())
                .current_dir(self.workspace);
            let ended = run(supervise::Job::from_command(command)
                .bounded(supervise::Limits::within(wall).keeping(self.limits.output_bytes)))?;
            match ended.ending {
                supervise::Ending::Exited(Some(0)) if ended.stdout.truncated => Err(format!(
                    "the tracker answered past the {}-byte output cap",
                    self.limits.output_bytes
                )),
                supervise::Ending::Exited(Some(0)) => Ok(ended.stdout.text),
                supervise::Ending::Exited(code) => {
                    let detail = scrub(&ended.stderr.text, self.secrets);
                    Err(format!(
                        "the tracker adapter exited {}: {}",
                        code.map_or_else(|| "on a signal".to_string(), |code| code.to_string()),
                        detail.trim().chars().take(500).collect::<String>()
                    ))
                }
                supervise::Ending::TimedOut => {
                    Err("the tracker fetch ran past its deadline".to_string())
                }
                supervise::Ending::Failed(why) => Err(format!(
                    "the tracker fetch failed: {}",
                    scrub(&why, self.secrets)
                )),
            }
        }
    }

    /// Runs a supervised job to its end from synchronous code.
    ///
    /// The job's runtime lives on a worker thread of its own, so a caller
    /// that already sits inside a runtime does not nest one inside
    /// another — the same bridge `capability::bounded::run` uses. The
    /// supervisor still owns the whole contract: the process group, the
    /// deadline, the reap, and the capped streams.
    fn run(job: supervise::Job) -> Result<supervise::Ended, String> {
        let worker = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| format!("no runtime to supervise the fetch: {error}"))?;
            Ok(runtime.block_on(job.run()))
        });
        worker
            .join()
            .map_err(|_| "the fetch's supervising thread panicked".to_string())?
    }

    /// Scrubs secrets out of text an error might carry.
    ///
    /// Two passes, because a leak takes either shape: every secret the
    /// caller named is replaced verbatim, and anything shaped like a
    /// GitHub token — `ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`, or
    /// `github_pat_` followed by token characters — is replaced whether
    /// or not the caller knew to name it. The result is what an error or
    /// a trace may safely hold.
    #[must_use]
    pub fn scrub(text: &str, secrets: &[String]) -> String {
        let mut out = text.to_string();
        for secret in secrets {
            if !secret.is_empty() {
                out = out.replace(secret.as_str(), "[redacted]");
            }
        }
        const PREFIXES: &[&str] = &["github_pat_", "ghp_", "gho_", "ghu_", "ghs_", "ghr_"];
        for prefix in PREFIXES {
            let mut from = 0;
            while let Some(at) = out[from..].find(prefix) {
                let start = from + at;
                let end = out[start..]
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .map(|tail| start + tail)
                    .unwrap_or(out.len());
                if end - start > prefix.len() + 4 {
                    out.replace_range(start..end, "[redacted]");
                }
                from = start + prefix.len();
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability;

    fn issue(number: u64) -> Issue {
        Issue {
            number,
            repo: "OpenAgentsInc/openagents".to_string(),
            node: format!("I_node_{number}"),
            title: format!("Issue {number}"),
            state: "open".to_string(),
            updated: "2026-09-20T00:00:00Z".to_string(),
            url: format!("https://github.com/OpenAgentsInc/openagents/issues/{number}"),
            body_digest: capability::digest_bytes(format!("body {number}").as_bytes()),
            excerpt: Some(format!("body {number}")),
            blocked_by: Vec::new(),
            blocked_by_complete: true,
            task: Some(TaskDef {
                prompt: format!("Work issue #{number}."),
                ..TaskDef::default()
            }),
        }
    }

    fn snapshot(issues: Vec<Issue>) -> Snapshot {
        Snapshot {
            v: SNAPSHOT_VERSION,
            kind: SNAPSHOT_KIND.to_string(),
            scope: Scope::github("OpenAgentsInc", "openagents", 16),
            base: Base {
                branch: "main".to_string(),
                revision: "1e84bb108a5c03f1d865bbcf12d9572f282e5eb5".to_string(),
            },
            fetched: "1758400000".to_string(),
            issues,
            skipped: Vec::new(),
        }
    }

    #[test]
    fn a_snapshot_round_trips_and_digests_deterministically() {
        let snapshot = snapshot(vec![issue(9427), issue(9507)]);
        let text = serde_json::to_string_pretty(&snapshot).unwrap();
        let read = Snapshot::parse(&text).expect("the snapshot reads back");
        assert_eq!(read.digest(), snapshot.digest());
        assert_eq!(read.issues.len(), 2);
    }

    #[test]
    fn a_snapshot_that_cannot_be_trusted_is_refused() {
        let mut wrong = snapshot(vec![issue(1)]);
        wrong.v = 7;
        assert!(wrong.validate().is_err());
        let mut kind = snapshot(vec![issue(1)]);
        kind.kind = "jira".to_string();
        assert!(kind.validate().unwrap_err().contains("jira"));
        let mut twice = snapshot(vec![issue(1), issue(1)]);
        twice.issues[1].node = "other".to_string();
        assert!(twice.validate().unwrap_err().contains("twice"));
        let mut self_dep = snapshot(vec![issue(1)]);
        self_dep.issues[0].blocked_by.push(Dependency {
            number: 1,
            ..Dependency::default()
        });
        assert!(self_dep.validate().unwrap_err().contains("itself"));
    }

    #[test]
    fn a_source_scoped_elsewhere_is_refused() {
        let snapshot = snapshot(vec![issue(1)]);
        snapshot
            .check_scope(&Scope::github("OpenAgentsInc", "openagents", 16))
            .expect("the scope matches");
        let reason = snapshot
            .check_scope(&Scope::github("OpenAgentsInc", "openagents", 17))
            .expect_err("a different project is a different selection");
        assert!(reason.contains("project 16"), "{reason}");
    }

    #[test]
    fn the_ready_and_the_blocked_are_both_recorded() {
        let mut blocked = issue(9507);
        blocked.task = None;
        let mut held = issue(9508);
        held.blocked_by.push(Dependency {
            number: 9507,
            state: "open".to_string(),
            ..Dependency::default()
        });
        let ready = issue(9509);
        let snapshot = snapshot(vec![blocked, held, ready]);

        let work = snapshot.work().expect("the snapshot renders work");
        assert_eq!(work.len(), 3, "blocked items stay visible to the lookup");
        assert_eq!(
            work[0].blocked.as_deref(),
            Some("no task definition names it")
        );
        assert!(
            work[1].blocked.is_none(),
            "an open in-list dependency is an after edge, not a block"
        );
        assert_eq!(work[1].after, ["9507"]);
        assert!(work[2].blocked.is_none());
    }

    #[test]
    fn every_blocker_kind_is_named() {
        let mut foreign = issue(1);
        foreign.repo = "OpenAgentsInc/elsewhere".to_string();
        let mut incomplete = issue(2);
        incomplete.blocked_by_complete = false;
        let mut cross_repo = issue(3);
        cross_repo.blocked_by.push(Dependency {
            number: 5,
            repo: Some("other/repo".to_string()),
            state: "closed".to_string(),
            ..Dependency::default()
        });
        let mut outside = issue(4);
        outside.blocked_by.push(Dependency {
            number: 9999,
            state: "open".to_string(),
            ..Dependency::default()
        });
        let mut unknown = issue(5);
        unknown.blocked_by.push(Dependency {
            number: 8888,
            state: String::new(),
            ..Dependency::default()
        });
        let snapshot = snapshot(vec![foreign, incomplete, cross_repo, outside, unknown]);

        let work = snapshot.work().unwrap();
        let reasons: Vec<&str> = work
            .iter()
            .map(|item| item.blocked.as_deref().unwrap_or(""))
            .collect();
        assert!(reasons[0].contains("outside the scoped"), "{}", reasons[0]);
        assert!(
            reasons[1].contains("completeness is unknown"),
            "{}",
            reasons[1]
        );
        assert!(
            reasons[2].contains("outside the scoped repository"),
            "{}",
            reasons[2]
        );
        assert!(
            reasons[3].contains("outside this selection"),
            "{}",
            reasons[3]
        );
        assert!(reasons[4].contains("does not pin"), "{}", reasons[4]);
    }

    #[test]
    fn a_cycle_blocks_every_member() {
        let mut a = issue(1);
        a.blocked_by.push(Dependency {
            number: 2,
            state: "open".to_string(),
            ..Dependency::default()
        });
        let mut b = issue(2);
        b.blocked_by.push(Dependency {
            number: 1,
            state: "open".to_string(),
            ..Dependency::default()
        });
        let snapshot = snapshot(vec![a, b, issue(3)]);

        let work = snapshot.work().unwrap();
        assert!(work[0].blocked.as_deref().unwrap_or("").contains("cycle"));
        assert!(work[1].blocked.as_deref().unwrap_or("").contains("cycle"));
        assert!(work[2].blocked.is_none());
    }

    #[test]
    fn a_closed_dependency_is_satisfied() {
        let mut closed = issue(1);
        closed.state = "closed".to_string();
        let mut ready = issue(2);
        ready.blocked_by.push(Dependency {
            number: 1,
            state: "closed".to_string(),
            ..Dependency::default()
        });
        ready.blocked_by.push(Dependency {
            number: 7000,
            state: "closed".to_string(),
            ..Dependency::default()
        });
        let snapshot = snapshot(vec![closed, ready]);

        let work = snapshot.work().unwrap();
        assert_eq!(work.len(), 1, "a closed issue is evidence, not work");
        assert!(work[0].blocked.is_none());
        assert_eq!(work[0].after, ["1", "7000"]);
    }

    #[test]
    fn the_rendered_list_is_the_contract_itself() {
        let mut held = issue(2);
        held.blocked_by.push(Dependency {
            number: 1,
            state: "open".to_string(),
            ..Dependency::default()
        });
        let snapshot = snapshot(vec![issue(1), held]);
        let list = snapshot.to_list().unwrap();

        assert_eq!(list["v"], json!(source::LIST_VERSION));
        assert_eq!(list["work"][1]["after"], json!(["1"]));
        let prompt = list["work"][0]["prompt"].as_str().unwrap();
        assert!(prompt.starts_with("Work issue #1."));
        assert!(prompt.contains("Untrusted tracker context"));
        assert!(prompt.contains("Issue OpenAgentsInc/openagents#1: Issue 1"));
    }

    #[test]
    fn a_missing_task_is_a_blocker_and_so_is_an_empty_prompt() {
        let mut no_task = issue(1);
        no_task.task = None;
        let mut empty = issue(2);
        empty.task = Some(TaskDef::default());
        let malformed = snapshot(vec![no_task, empty]);
        // The empty prompt fails validation at parse — a task that asks
        // nothing is a malformed snapshot, not a blocked item.
        assert!(malformed.validate().unwrap_err().contains("asks nothing"));

        let ready = snapshot(vec![issue(3)]);
        assert!(ready.work().unwrap()[0].blocked.is_none());
    }

    #[test]
    fn revalidation_names_every_drift() {
        let pinned = snapshot(vec![issue(1), issue(2)]);
        let mut fresh = snapshot(vec![issue(1), issue(2), issue(3)]);
        fresh.issues[0].updated = "2026-09-20T01:00:00Z".to_string();
        fresh.issues[1].state = "closed".to_string();
        fresh.issues[1].updated = "2026-09-20T02:00:00Z".to_string();
        fresh.base.revision = "0a1e2c9000000000000000000000000000000000".to_string();

        let drift = pinned.revalidate(&fresh);
        let reasons: Vec<String> = drift
            .iter()
            .map(|d| format!("{} {}", d.item, d.reason))
            .collect();
        assert!(
            reasons
                .iter()
                .any(|r| r.contains("base") && r.contains("moved")),
            "{reasons:?}"
        );
        assert!(
            reasons
                .iter()
                .any(|r| r.contains("#1") && r.contains("changed")),
            "{reasons:?}"
        );
        assert!(
            reasons
                .iter()
                .any(|r| r.contains("#2") && r.contains("closed")),
            "{reasons:?}"
        );
        assert!(
            reasons
                .iter()
                .any(|r| r.contains("#3") && r.contains("appeared")),
            "{reasons:?}"
        );

        assert!(pinned.revalidate(&pinned.clone()).is_empty());
    }

    #[test]
    fn a_gone_issue_and_a_recreated_issue_are_drift() {
        let pinned = snapshot(vec![issue(1), issue(2)]);
        let mut fresh = snapshot(vec![issue(2)]);
        fresh.issues[0].node = "I_recreated".to_string();

        let drift = pinned.revalidate(&fresh);
        assert!(
            drift
                .iter()
                .any(|d| d.item == "#1" && d.reason.contains("no longer"))
        );
        assert!(
            drift
                .iter()
                .any(|d| d.item == "#2" && d.reason.contains("re-created"))
        );
    }

    #[test]
    fn a_task_map_reads_and_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tasks.json");
        std::fs::write(
            &path,
            r#"{"v":1,"tasks":{"9507":{"prompt":"Implement the tracker intake.","touches":["crates/coder/src/tracker.rs"],"writes":true,"expects":"done"}}}"#,
        )
        .unwrap();
        let map = TaskMap::load(&path).unwrap();
        let task = map.tasks.get(&9507).unwrap();
        assert!(task.writes);
        assert_eq!(task.expects.as_deref(), Some("done"));

        std::fs::write(&path, r#"{"v":9,"tasks":{}}"#).unwrap();
        assert!(TaskMap::load(&path).unwrap_err().contains("version"));
        std::fs::write(
            &path,
            r#"{"v":1,"tasks":{"1":{"prompt":"x","touches":["../outside.rs"]}}}"#,
        )
        .unwrap();
        assert!(TaskMap::load(&path).unwrap_err().contains("workspace"));
    }

    mod github {
        use std::collections::BTreeMap;

        use serde_json::{Value, json};

        use super::super::github::{Fetched, assemble, page, scrub};
        use super::super::{Base, Limits, Scope, TASKS_VERSION, TaskDef, TaskMap};
        use crate::capability;

        fn page_json(nodes: Value, has_next: bool) -> String {
            json!({
                "data": {
                    "organization": {"projectV2": {"title": "P", "items": {
                        "pageInfo": {"hasNextPage": has_next, "endCursor": if has_next { json!("c2") } else { Value::Null }},
                        "nodes": nodes
                    }}},
                    "repository": {"defaultBranchRef": {"name": "main", "target": {"oid": "1e84bb108a5c03f1d865bbcf12d9572f282e5eb5"}}}
                }
            })
            .to_string()
        }

        fn issue_node(number: u64) -> Value {
            json!({
                "id": format!("PVTI_{number}"),
                "content": {
                    "__typename": "Issue",
                    "id": format!("I_node_{number}"),
                    "number": number,
                    "title": format!("Issue {number}"),
                    "state": "OPEN",
                    "updatedAt": "2026-09-20T00:00:00Z",
                    "url": format!("https://github.com/OpenAgentsInc/openagents/issues/{number}"),
                    "body": format!("body {number}"),
                    "repository": {"nameWithOwner": "OpenAgentsInc/openagents"},
                    "blockedBy": {"pageInfo": {"hasNextPage": false}, "nodes": []}
                }
            })
        }

        #[test]
        fn a_page_parses_and_carries_its_cursor() {
            let first = page(&page_json(json!([issue_node(1), issue_node(2)]), true)).unwrap();
            assert_eq!(first.issues.len(), 2);
            assert_eq!(first.next.as_deref(), Some("c2"));
            assert_eq!(
                first.base.unwrap().revision,
                "1e84bb108a5c03f1d865bbcf12d9572f282e5eb5"
            );

            let last = page(&page_json(json!([issue_node(3)]), false)).unwrap();
            assert!(last.next.is_none());
        }

        #[test]
        fn errors_and_missing_data_refuse_the_page() {
            let errors =
                json!({"errors": [{"message": "Could not resolve to a ProjectV2"}]}).to_string();
            assert!(page(&errors).unwrap_err().contains("Could not resolve"));
            assert!(page("{}").is_err());
            let no_project = json!({"data": {
                "organization": {"projectV2": null},
                "repository": {"defaultBranchRef": {"name": "main", "target": {"oid": "1e84bb108a5c03f1d865bbcf12d9572f282e5eb5"}}}
            }}).to_string();
            assert!(page(&no_project).unwrap_err().contains("no project"));
            let no_repo = json!({"data": {"organization": {"projectV2": {}}, "repository": null}})
                .to_string();
            assert!(page(&no_repo).unwrap_err().contains("repository"));
        }

        #[test]
        fn nonissue_items_are_skipped_and_named() {
            let nodes = json!([
                issue_node(1),
                {"id": "PVTI_draft", "content": {"__typename": "DraftIssue", "title": "a draft"}},
                {"id": "PVTI_pr", "content": {"__typename": "PullRequest", "number": 9}}
            ]);
            let page = page(&page_json(nodes, false)).unwrap();
            assert_eq!(page.issues.len(), 1);
            assert_eq!(page.skipped.len(), 2);
            assert!(page.skipped[0].reason.contains("DraftIssue"));
        }

        #[test]
        fn inaccessible_items_and_missing_dependency_completeness_refuse() {
            assert!(
                page(&page_json(
                    json!([{"id":"unreadable","content":null}]),
                    false
                ))
                .is_err()
            );
            let mut value = serde_json::to_value(super::issue(1)).unwrap();
            value.as_object_mut().unwrap().remove("blocked_by_complete");
            let issue: super::super::Issue = serde_json::from_value(value).unwrap();
            assert!(!issue.blocked_by_complete);
            assert!(
                super::snapshot(vec![issue]).work().unwrap()[0]
                    .blocked
                    .is_some()
            );
        }

        #[test]
        fn pagination_refuses_loops_duplicates_and_skipped_item_overflow() {
            use super::super::github::paginate;
            let limits = Limits::bounded();
            let mut calls = 0;
            let result = paginate(&limits, |_, _| {
                calls += 1;
                page(&page_json(json!([issue_node(calls)]), true))
            });
            assert!(result.unwrap_err().contains("cursor"));
            assert_eq!(calls, 2);
            let mut calls = 0;
            let result = paginate(&limits, |_, _| {
                calls += 1;
                page(&page_json(json!([issue_node(1)]), calls == 1))
            });
            assert!(result.unwrap_err().contains("twice"));
            let limits = Limits { items: 1, ..limits };
            let result = paginate(&limits, |_, _| {
                page(&page_json(
                    json!([
                        {"id":"draft-a","content":{"__typename":"DraftIssue"}},
                        {"id":"draft-b","content":{"__typename":"DraftIssue"}}
                    ]),
                    false,
                ))
            });
            assert!(result.unwrap_err().contains("items"));
        }

        #[test]
        fn oversized_files_are_rejected_during_capture() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("oversized.json");
            let file = std::fs::File::create(&path).unwrap();
            file.set_len(super::super::SNAPSHOT_BYTES + 1).unwrap();
            assert!(
                super::super::Snapshot::load(&path)
                    .unwrap_err()
                    .contains("past it")
            );
            assert!(TaskMap::load(&path).unwrap_err().contains("past it"));
        }

        #[test]
        fn a_missing_field_and_a_cut_dependency_list_are_caught() {
            let mut thin = issue_node(3);
            thin["content"].as_object_mut().unwrap().remove("updatedAt");
            assert!(page(&page_json(json!([thin]), false)).is_err());

            let mut cut = issue_node(4);
            cut["content"]["blockedBy"]["pageInfo"]["hasNextPage"] = json!(true);
            let page = page(&page_json(json!([cut]), false)).unwrap();
            assert!(!page.issues[0].blocked_by_complete);
        }

        #[test]
        fn a_dependency_parses_with_its_repository_and_state() {
            let mut held = issue_node(5);
            held["content"]["blockedBy"]["nodes"] = json!([
                {"number": 1, "state": "CLOSED", "updatedAt": "t",
                 "repository": {"nameWithOwner": "OpenAgentsInc/openagents"}},
                {"number": 2, "state": "OPEN",
                 "repository": {"nameWithOwner": "other/repo"}}
            ]);
            let page = page(&page_json(json!([held]), false)).unwrap();
            let deps = &page.issues[0].blocked_by;
            assert_eq!(deps[0].state, "closed");
            assert_eq!(deps[0].repo.as_deref(), Some("OpenAgentsInc/openagents"));
            assert_eq!(deps[1].repo.as_deref(), Some("other/repo"));
        }

        #[test]
        fn assembly_refuses_duplicates_and_honors_the_item_bound() {
            let one = Fetched {
                number: 1,
                repo: "OpenAgentsInc/openagents".to_string(),
                node: "I1".to_string(),
                title: "one".to_string(),
                state: "open".to_string(),
                updated: "t".to_string(),
                url: String::new(),
                body: "body".to_string(),
                blocked_by: Vec::new(),
                blocked_by_complete: true,
            };
            let mut two = one.clone();
            two.number = 2;
            two.node = "I2".to_string();
            let scope = Scope::github("OpenAgentsInc", "openagents", 16);
            let base = Base {
                branch: "main".to_string(),
                revision: "abc".to_string(),
            };
            let tasks = TaskMap {
                v: TASKS_VERSION,
                tasks: BTreeMap::from([(
                    1,
                    TaskDef {
                        prompt: "Do one.".to_string(),
                        ..TaskDef::default()
                    },
                )]),
            };
            let snapshot = assemble(
                scope.clone(),
                vec![one.clone(), two.clone()],
                Vec::new(),
                base.clone(),
                &tasks,
                &Limits::bounded(),
                "1",
            )
            .unwrap();
            assert_eq!(snapshot.issues.len(), 2);
            assert!(
                snapshot.issues[1].task.is_none(),
                "issue 2 has no task definition"
            );
            assert_eq!(
                snapshot.issues[0].body_digest,
                capability::digest_bytes(b"body")
            );

            let mut dup = one.clone();
            dup.node = "I1b".to_string();
            assert!(
                assemble(
                    scope.clone(),
                    vec![one.clone(), dup],
                    Vec::new(),
                    base.clone(),
                    &tasks,
                    &Limits::bounded(),
                    "1"
                )
                .is_err()
            );

            let tight = Limits {
                items: 1,
                ..Limits::bounded()
            };
            assert!(
                assemble(scope, vec![one, two], Vec::new(), base, &tasks, &tight, "1")
                    .unwrap_err()
                    .contains("bound")
            );
        }

        #[test]
        fn secrets_are_scrubbed_from_errors() {
            let secrets = vec![
                "ghp_secretvalue123".to_string(),
                "another-secret".to_string(),
            ];
            let text =
                "fatal: ghp_secretvalue123 and github_pat_11ABCDEFG and another-secret failed";
            let scrubbed = scrub(text, &secrets);
            assert!(!scrubbed.contains("secretvalue"), "{scrubbed}");
            assert!(!scrubbed.contains("11ABCDEFG"), "{scrubbed}");
            assert!(!scrubbed.contains("another-secret"), "{scrubbed}");
            assert!(scrubbed.contains("[redacted]"));
        }
    }
}
