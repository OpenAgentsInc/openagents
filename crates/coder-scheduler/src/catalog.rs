//! The pinned task catalog.
//!
//! A catalog is the backlog as a document: a versioned, self-digested
//! list of tasks, each pinned to a stable id, an issue number, the base
//! it was cut against, the input it was given, its dependencies, its
//! declared read and write footprints, its priority, and the resources
//! it will hold while admitted. The scheduler plans only over a verified
//! catalog — a document whose digest does not recompute, or whose
//! structure is defective, refuses to load rather than plan over a guess.
//!
//! # What validation catches
//!
//! - A duplicated task id, or a dependency listed twice.
//! - A dependency cycle. A cycle cannot complete, so the catalog that
//!   contains one does not load.
//! - A footprint path that escapes the tree — absolute paths, `..`
//!   segments, `.` segments, empty segments, or a backslash, which is a
//!   separator on some platforms and a literal on others.
//!
//! A dependency on a task the catalog does not contain is *not* a
//! defect: the task may have completed outside this run. At plan time an
//! absent dependency is `unknown` unless the caller supplies it as
//! externally completed — absent is never assumed complete.
//!
//! # Footprints
//!
//! A task declares the paths it reads and the paths it writes. Two
//! footprints conflict when one task's writes reach a path the other
//! task reads or writes — where *reach* includes ancestors, so a write
//! to `crates/` conflicts with a read of `crates/jev/src/lib.rs`.
//! Shared reads do not conflict. A task that cannot enumerate its paths
//! declares [`Footprint::Unknown`], which conflicts with everything,
//! because a footprint nobody can state is treated as touching anything.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::resources::Resources;

/// The schema tag a catalog carries.
pub const SCHEMA: &str = "openagents.scheduler.catalog.v1";

/// The catalog: a named, digested list of pinned tasks.
///
/// `digest` covers every field but itself, canonicalized key-sorted JSON
/// over SHA-256 — the same self-verifying shape the receipts and the
/// tenancy manifest use. A reader recomputes it before planning; a
/// tampered catalog refuses on load.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Catalog {
    /// The schema tag.
    pub v: String,
    /// The backlog's name, for the operator's record.
    pub name: String,
    /// The tasks, in the catalog's declared order. The plan sorts them;
    /// this order is authorship, not scheduling.
    pub tasks: Vec<Task>,
    /// The digest over every field above.
    pub digest: String,
}

/// One pinned task.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Task {
    /// The stable id — the join key for dependencies, ledger state, and
    /// external completion. It never names a different task.
    pub id: String,
    /// The issue number the task was cut from, for the record.
    pub issue: u64,
    /// The base the task branches from — a commit or manifest identity.
    /// A result produced off a different base is not this task's result.
    pub base: String,
    /// The input's identity — the digest of the worklist entry, prompt,
    /// or item the task runs. Together with `base` it is the task's
    /// *content identity*: change either and old results stop applying.
    pub input: String,
    /// Stable ids this task depends on. Each must complete — in the
    /// ledger, or supplied as externally completed — before this task is
    /// ready. An absent id is `unknown`, never assumed complete.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// What the task touches.
    pub footprint: Footprint,
    /// Scheduling priority; higher runs earlier. Ordering within one
    /// priority is issue number, then id — stable, not arrival order.
    #[serde(default)]
    pub priority: i64,
    /// The resource vector held while admitted.
    pub resources: Resources,
    /// The stated duration, in simulation ticks. Real scheduling does
    /// not read it; [`crate::simulate`] does.
    #[serde(default = "one_tick")]
    pub estimate_ticks: u64,
}

/// What a task touches.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Footprint {
    /// Every path the task reads or writes is listed.
    Declared {
        /// Paths the task reads.
        #[serde(default)]
        reads: Vec<String>,
        /// Paths the task writes.
        #[serde(default)]
        writes: Vec<String>,
    },
    /// The task cannot enumerate its paths. Conservative: it conflicts
    /// with every other footprint, because anything might be touched.
    Unknown,
}

/// How two footprints collide.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictKind {
    /// Both sides write an overlapping path.
    WriteWrite,
    /// One side writes a path the other reads.
    WriteRead,
}

/// A found collision between two footprints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Conflict {
    /// The colliding path on the first side.
    pub path_a: String,
    /// The colliding path on the second side, when the side named one.
    /// An [`Footprint::Unknown`] side names nothing.
    pub path_b: Option<String>,
    /// How they collide.
    pub kind: ConflictKind,
}

/// Why a catalog refused to load.
#[derive(Debug)]
pub enum CatalogError {
    /// The document did not parse.
    Malformed(String),
    /// The schema tag is not this version's.
    UnknownSchema(String),
    /// The digest does not recompute over the contents.
    Tampered,
    /// Two tasks share a stable id.
    DuplicateId(String),
    /// A task lists the same dependency twice.
    DuplicateDependency { task: String, dependency: String },
    /// A dependency cycle — the tasks on the cycle, in order.
    Cycle(Vec<String>),
    /// A footprint path escapes the tree or is malformed.
    BadPath {
        /// The task that declares it.
        task: String,
        /// The path as written.
        path: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// A task with no id, or a dependency on the empty id, names nothing.
    EmptyId,
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(error) => write!(f, "malformed catalog: {error}"),
            Self::UnknownSchema(schema) => {
                write!(f, "catalog schema `{schema}` is not `{SCHEMA}`")
            }
            Self::Tampered => {
                write!(
                    f,
                    "the catalog's contents do not match its digest, so it was changed after it was sealed"
                )
            }
            Self::DuplicateId(id) => write!(f, "two tasks share id `{id}`"),
            Self::DuplicateDependency { task, dependency } => {
                write!(f, "task `{task}` lists dependency `{dependency}` twice")
            }
            Self::Cycle(tasks) => write!(f, "dependency cycle: {}", tasks.join(" -> ")),
            Self::BadPath { task, path, reason } => {
                write!(f, "task `{task}` declares path `{path}`: {reason}")
            }
            Self::EmptyId => write!(f, "a task or a dependency has an empty id"),
        }
    }
}

impl std::error::Error for CatalogError {}

impl Catalog {
    /// Begin an unsigned catalog. `digest` stays empty until
    /// [`Catalog::seal`].
    #[must_use]
    pub fn new(name: impl Into<String>, tasks: Vec<Task>) -> Self {
        Self {
            v: SCHEMA.to_string(),
            name: name.into(),
            tasks,
            digest: String::new(),
        }
    }

    /// Fill in `digest` over the catalog's other fields.
    ///
    /// Validation runs first: a defective catalog has no meaningful
    /// digest because it cannot be planned over.
    pub fn seal(&mut self) -> Result<(), CatalogError> {
        self.validate()?;
        self.digest = self.compute_digest();
        Ok(())
    }

    /// The digest over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a catalog serializes");
        value
            .as_object_mut()
            .expect("a catalog is an object")
            .remove("digest");
        digest_value(&value)
    }

    /// Read and self-check a catalog's text: schema, digest, structure.
    pub fn parse(text: &str) -> Result<Self, CatalogError> {
        let catalog: Self = serde_json::from_str(text)
            .map_err(|error| CatalogError::Malformed(error.to_string()))?;
        if catalog.v != SCHEMA {
            return Err(CatalogError::UnknownSchema(catalog.v.clone()));
        }
        catalog.validate()?;
        if catalog.digest != catalog.compute_digest() {
            return Err(CatalogError::Tampered);
        }
        Ok(catalog)
    }

    /// Serialize the sealed catalog.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a catalog serializes")
    }

    /// The structural checks: ids unique, dependencies listed once,
    /// footprints inside the tree, and no dependency cycles.
    pub fn validate(&self) -> Result<(), CatalogError> {
        let mut ids = BTreeSet::new();
        for task in &self.tasks {
            if task.id.is_empty() {
                return Err(CatalogError::EmptyId);
            }
            if !ids.insert(task.id.as_str()) {
                return Err(CatalogError::DuplicateId(task.id.clone()));
            }
            let mut seen = BTreeSet::new();
            for dependency in &task.depends_on {
                if dependency.is_empty() {
                    return Err(CatalogError::EmptyId);
                }
                if !seen.insert(dependency.as_str()) {
                    return Err(CatalogError::DuplicateDependency {
                        task: task.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
            task.check_paths()?;
        }
        self.acyclic()
    }

    /// Depth-first cycle check over the catalog's dependency edges.
    /// Dependencies on absent ids are not edges — they are `unknown` at
    /// plan time, not a defect here.
    fn acyclic(&self) -> Result<(), CatalogError> {
        let by_id: BTreeMap<&str, &Task> = self
            .tasks
            .iter()
            .map(|task| (task.id.as_str(), task))
            .collect();
        // 0 = unvisited, 1 = on the walk, 2 = done.
        let mut marks: BTreeMap<&str, u8> = BTreeMap::new();
        let mut walk: Vec<&str> = Vec::new();
        for task in &self.tasks {
            visit(task.id.as_str(), &by_id, &mut marks, &mut walk)?;
        }
        Ok(())
    }

    /// The task under one stable id.
    #[must_use]
    pub fn task(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|task| task.id == id)
    }
}

impl Task {
    /// The task's content identity — a digest over the whole task, so
    /// changing any field repins it. The ledger binds it at dispatch and
    /// checks it at acceptance, so a task changed after its attempt ran
    /// cannot accept the old attempt's result.
    #[must_use]
    pub fn digest(&self) -> String {
        digest_value(&serde_json::to_value(self).expect("a task serializes"))
    }

    /// The declared footprint paths must stay inside the tree.
    fn check_paths(&self) -> Result<(), CatalogError> {
        if let Footprint::Declared { reads, writes } = &self.footprint {
            for path in reads.iter().chain(writes.iter()) {
                check_path(path).map_err(|reason| CatalogError::BadPath {
                    task: self.id.clone(),
                    path: path.clone(),
                    reason,
                })?;
            }
        }
        Ok(())
    }
}

/// One node's place in the depth-first walk. Reaching a node already on
/// the walk returns the cycle, from the node's first occurrence back to
/// itself.
fn visit<'a>(
    id: &'a str,
    by_id: &BTreeMap<&'a str, &'a Task>,
    marks: &mut BTreeMap<&'a str, u8>,
    walk: &mut Vec<&'a str>,
) -> Result<(), CatalogError> {
    match marks.get(id).copied().unwrap_or(0) {
        2 => return Ok(()),
        1 => {
            let at = walk.iter().position(|on| *on == id).unwrap_or(0);
            let mut cycle: Vec<String> = walk[at..].iter().map(|id| (*id).to_string()).collect();
            cycle.push(id.to_string());
            return Err(CatalogError::Cycle(cycle));
        }
        _ => {}
    }
    marks.insert(id, 1);
    walk.push(id);
    if let Some(node) = by_id.get(id) {
        for dependency in &node.depends_on {
            visit(dependency.as_str(), by_id, marks, walk)?;
        }
    }
    walk.pop();
    marks.insert(id, 2);
    Ok(())
}

fn one_tick() -> u64 {
    1
}

/// One footprint path must be a normalized relative path.
///
/// Rejected: empty, absolute, anchored on `~`, containing `.` or `..`
/// or empty segments, or carrying a backslash — a separator on some
/// platforms and a literal byte on others, so it is ambiguous by
/// construction. A task that means the whole tree declares
/// [`Footprint::Unknown`], not `.`.
fn check_path(path: &str) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("the path is empty");
    }
    if path.starts_with('/') {
        return Err("the path is absolute; use a path relative to the repository root");
    }
    if path.starts_with('~') {
        return Err("a path that starts with `~` points outside the repository");
    }
    if path.contains('\\') {
        return Err("the path contains a backslash; use `/` as the separator");
    }
    if path.contains('\0') {
        return Err("a NUL byte cannot appear in a path");
    }
    for segment in path.split('/') {
        match segment {
            "" => return Err("the path contains an empty segment, such as `//`"),
            "." => return Err("the path contains a `.` segment; remove it"),
            ".." => return Err("the path contains `..`, which can point outside the repository"),
            _ => {}
        }
    }
    Ok(())
}

/// Do two normalized paths reach the same place — equal, or one an
/// ancestor of the other by path segment?
#[must_use]
pub fn paths_overlap(a: &str, b: &str) -> bool {
    let a: Vec<&str> = a.split('/').collect();
    let b: Vec<&str> = b.split('/').collect();
    let shared = a.len().min(b.len());
    a[..shared] == b[..shared]
}

/// The first collision between two footprints, in a deterministic
/// order: the first side's writes against the second's writes, then the
/// second's reads; then the second's writes against the first's reads.
/// Shared reads never collide.
#[must_use]
pub fn footprints_conflict(a: &Footprint, b: &Footprint) -> Option<Conflict> {
    let (
        Footprint::Declared {
            reads: a_reads,
            writes: a_writes,
        },
        Footprint::Declared {
            reads: b_reads,
            writes: b_writes,
        },
    ) = (a, b)
    else {
        // At least one side cannot enumerate its paths — that side is
        // treated as touching anything, so the footprints collide.
        return Some(Conflict {
            path_a: "<unknown>".to_string(),
            path_b: None,
            kind: ConflictKind::WriteWrite,
        });
    };
    for write in a_writes {
        for other in b_writes {
            if paths_overlap(write, other) {
                return Some(Conflict {
                    path_a: write.clone(),
                    path_b: Some(other.clone()),
                    kind: ConflictKind::WriteWrite,
                });
            }
        }
        for other in b_reads {
            if paths_overlap(write, other) {
                return Some(Conflict {
                    path_a: write.clone(),
                    path_b: Some(other.clone()),
                    kind: ConflictKind::WriteRead,
                });
            }
        }
    }
    for write in b_writes {
        for other in a_reads {
            if paths_overlap(write, other) {
                return Some(Conflict {
                    path_a: other.clone(),
                    path_b: Some(write.clone()),
                    kind: ConflictKind::WriteRead,
                });
            }
        }
    }
    None
}

/// The digest of a JSON value's canonical form — keys sorted, no
/// whitespace, the same canonicalization the workspace's digests share.
fn digest_value(value: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonicalize(value).as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// Canonical JSON: keys sorted, whitespace gone. Keys are sorted here
/// rather than trusted to the map, so the digest does not depend on who
/// wrote the bytes.
fn canonicalize(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("a key serializes"));
                out.push(':');
                out.push_str(&canonicalize(&map[*key]));
            }
            out.push('}');
            out
        }
        Value::Array(items) => {
            let mut out = String::from("[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&canonicalize(item));
            }
            out.push(']');
            out
        }
        other => serde_json::to_string(other).expect("a value serializes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str) -> Task {
        Task {
            id: id.to_string(),
            issue: 1,
            base: "sha256:base".to_string(),
            input: "sha256:input".to_string(),
            depends_on: vec![],
            footprint: Footprint::Declared {
                reads: vec![],
                writes: vec![format!("crates/{id}/src/lib.rs")],
            },
            priority: 0,
            resources: Resources::default(),
            estimate_ticks: 1,
        }
    }

    fn sealed(tasks: Vec<Task>) -> Catalog {
        let mut catalog = Catalog::new("test", tasks);
        catalog.seal().unwrap();
        catalog
    }

    #[test]
    fn a_sealed_catalog_round_trips_and_verifies() {
        let catalog = sealed(vec![task("a"), task("b")]);
        let parsed = Catalog::parse(&catalog.to_json()).unwrap();
        assert_eq!(parsed.digest, catalog.digest);
    }

    #[test]
    fn a_tampered_catalog_refuses() {
        let catalog = sealed(vec![task("a")]);
        let text = catalog.to_json().replacen("\"a\"", "\"z\"", 1);
        assert!(matches!(
            Catalog::parse(&text),
            Err(CatalogError::Tampered | CatalogError::Malformed(_))
        ));
    }

    #[test]
    fn duplicate_ids_refuse() {
        let mut catalog = Catalog::new("test", vec![task("a"), task("a")]);
        assert!(matches!(
            catalog.seal(),
            Err(CatalogError::DuplicateId(id)) if id == "a"
        ));
    }

    #[test]
    fn a_self_dependency_is_a_cycle() {
        let mut t = task("a");
        t.depends_on = vec!["a".to_string()];
        let mut catalog = Catalog::new("test", vec![t]);
        assert!(matches!(catalog.seal(), Err(CatalogError::Cycle(_))));
    }

    #[test]
    fn an_indirect_cycle_is_detected() {
        let mut a = task("a");
        a.depends_on = vec!["b".to_string()];
        let mut b = task("b");
        b.depends_on = vec!["c".to_string()];
        let mut c = task("c");
        c.depends_on = vec!["a".to_string()];
        let mut catalog = Catalog::new("test", vec![a, b, c]);
        match catalog.seal() {
            Err(CatalogError::Cycle(path)) => {
                assert_eq!(path.first(), path.last(), "the cycle closes");
                assert_eq!(path.len(), 4);
            }
            other => panic!("expected a cycle, got {other:?}"),
        }
    }

    #[test]
    fn an_absent_dependency_is_not_a_defect() {
        let mut t = task("a");
        t.depends_on = vec!["external".to_string()];
        sealed(vec![t]);
    }

    #[test]
    fn escaping_paths_refuse() {
        for path in [
            "../outside.rs",
            "/abs/path.rs",
            "./dot.rs",
            "a//b.rs",
            "a/./b.rs",
            "~/.ssh/id",
            "back\\slash.rs",
            "",
        ] {
            let mut t = task("a");
            t.footprint = Footprint::Declared {
                reads: vec![],
                writes: vec![path.to_string()],
            };
            let mut catalog = Catalog::new("test", vec![t]);
            assert!(
                matches!(catalog.seal(), Err(CatalogError::BadPath { .. })),
                "path `{path}` must refuse"
            );
        }
    }

    #[test]
    fn an_ancestor_write_conflicts_with_a_nested_read() {
        let writer = Footprint::Declared {
            reads: vec![],
            writes: vec!["crates/jev".to_string()],
        };
        let reader = Footprint::Declared {
            reads: vec!["crates/jev/src/lib.rs".to_string()],
            writes: vec![],
        };
        let conflict = footprints_conflict(&writer, &reader).unwrap();
        assert_eq!(conflict.kind, ConflictKind::WriteRead);
    }

    #[test]
    fn shared_reads_do_not_conflict() {
        let a = Footprint::Declared {
            reads: vec!["Cargo.toml".to_string()],
            writes: vec!["crates/a/x.rs".to_string()],
        };
        let b = Footprint::Declared {
            reads: vec!["Cargo.toml".to_string()],
            writes: vec!["crates/b/y.rs".to_string()],
        };
        assert_eq!(footprints_conflict(&a, &b), None);
    }

    #[test]
    fn an_unknown_footprint_conflicts_with_everything() {
        let declared = Footprint::Declared {
            reads: vec![],
            writes: vec!["crates/a/x.rs".to_string()],
        };
        assert!(footprints_conflict(&Footprint::Unknown, &declared).is_some());
        assert!(footprints_conflict(&Footprint::Unknown, &Footprint::Unknown).is_some());
    }

    #[test]
    fn sibling_directories_do_not_overlap() {
        assert!(!paths_overlap("crates/jev", "crates/kev"));
        assert!(paths_overlap("crates/jev", "crates/jev/src"));
        assert!(paths_overlap("docs", "docs"));
        assert!(!paths_overlap("docs", "docsx"));
    }

    #[test]
    fn a_changed_task_digests_differently() {
        let before = task("a");
        let mut after = before.clone();
        after.input = "sha256:other-input".to_string();
        assert_ne!(before.digest(), after.digest());
    }
}
