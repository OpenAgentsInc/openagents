//! Parallel Microluna sessions in the suite loop (`microluna-v7`).
//!
//! v6 runs every session after the last one, on one workspace. Three
//! things now run at once where they can:
//!
//! 1. The first edit session works in the real workspace while
//!    `accept.define` writes the suite and proves it red on a snapshot
//!    (`accept::Inputs::target`).
//! 2. Several suite writers, each on a share of the requirements
//!    (`accept::Options::writers`).
//! 3. Edit sessions on red tests that don't touch the same files, each in
//!    its own copy of the workspace, merged back by code.
//!
//! This module holds what 3 needs besides the sessions themselves: the
//! units of red work and the files their evidence names ([`units`]),
//! Jev's question whether two units share a file ([`independence`]), how
//! units pack into lanes ([`lanes`]), and the three-way merge of each
//! lane's copy into the workspace ([`merge`]). It also holds the timeline
//! every suite loop is measured by ([`Track`], [`summary`]): which
//! sessions overlapped, the effective concurrency, the critical path, and
//! what the overlap saved.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use jev::{Noul, NoulCriteria, Questions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::accept::{AcceptanceSuite, RunResult};

/// The component Jev's independence question is recorded under.
pub const COMPONENT: &str = "microluna.parallel";

/// The ATIF step extension that carries a loop's parallel summary.
pub const SUMMARY_EXTENSION: &str = "microluna.parallel.v1";

/// The ATIF step extension on a session's delegation step: its group,
/// batch, and the sessions it ran beside.
pub const LANE_EXTENSION: &str = "microluna.lane";

/// Two units share a file, for planning, when Jev's probability that they
/// do is at least this. It errs toward sharing: a false "independent"
/// costs a conflict and a requeued session, a false "shared" only a
/// session that could have run beside another. Not calibrated yet.
pub const SHARED_MIN: f64 = 0.4;

/// The most units one plan considers; more red tests are folded into this
/// many consecutive units.
pub const MAX_UNITS: usize = 6;

/// The most workspace files listed, and read for names.
pub const MAX_FILES: usize = 4_000;

/// The largest workspace the host copies for lanes, in bytes.
pub const MAX_COPY_BYTES: u64 = 256 * 1024 * 1024;

/// Directories a lane's diff ignores: caches a test run rewrites, and the
/// Git database, which a merge of files doesn't carry.
pub const UNMERGED: [&str; 5] = [
    ".git",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
];

/// Directories the file list for evidence skips, besides [`UNMERGED`].
const UNLISTED: [&str; 6] = ["node_modules", "target", ".venv", "venv", "dist", "build"];

/// One unit of red work: red tests, the requirements they check, and the
/// workspace files their evidence names.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Unit {
    pub tests: Vec<String>,
    pub requirements: Vec<String>,
    /// Each test's `# what:` line.
    pub what: Vec<String>,
    /// Workspace files named in the tests' sources, their output, or the
    /// requirements' text.
    pub files: Vec<String>,
    /// The tail of each red test's output, for Jev.
    #[serde(skip)]
    pub output: String,
}

/// Whether the workspace is small enough to copy once per lane.
#[must_use]
pub fn copyable(dir: &Path) -> bool {
    let mut stack = vec![dir.to_path_buf()];
    let (mut files, mut bytes) = (0usize, 0u64);
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            return false;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            } else {
                files += 1;
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
                if files > 20_000 || bytes > MAX_COPY_BYTES {
                    return false;
                }
            }
        }
    }
    true
}

/// The workspace's files, relative to `dir`, sorted, without caches,
/// dependency trees, and build output, at most [`MAX_FILES`].
#[must_use]
pub fn workspace_files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !UNMERGED.contains(&name.as_str()) && !UNLISTED.contains(&name.as_str()) {
                    stack.push(path);
                }
            } else if let Ok(relative) = path.strip_prefix(dir) {
                out.push(relative.to_string_lossy().into_owned());
                if out.len() >= MAX_FILES {
                    out.sort();
                    return out;
                }
            }
        }
    }
    out.sort();
    out
}

/// Whether `needle` occurs in `text` as a whole name: not inside a longer
/// identifier or path segment.
fn contains_name(text: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut from = 0;
    while let Some(at) = text[from..].find(needle) {
        let start = from + at;
        let end = start + needle.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        if before.is_none_or(|c| !word(c)) && after.is_none_or(|c| !word(c)) {
            return true;
        }
        from = start + needle.len().max(1);
        if from >= text.len() {
            break;
        }
    }
    false
}

/// The files of `files` that `text` names: by relative path, by a file
/// name with an extension, or, for a Python module, by its dotted name.
#[must_use]
pub fn files_named(text: &str, files: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for file in files {
        let name = file.rsplit('/').next().unwrap_or(file);
        let dotted = file
            .strip_suffix(".py")
            .map(|module| module.replace('/', "."))
            .filter(|module| module.contains('.'));
        let named = contains_name(text, file)
            || (name.contains('.') && name.len() >= 4 && contains_name(text, name))
            || dotted.is_some_and(|module| contains_name(text, &module));
        if named {
            out.push(file.clone());
        }
    }
    out.truncate(24);
    out
}

/// The red tests of `result` as units, requeued tests first, then the
/// rest in suite order. More than [`MAX_UNITS`] red tests fold into that
/// many consecutive units. `requirement_text` gives a requirement's words.
#[must_use]
pub fn units(
    suite: &AcceptanceSuite,
    result: &RunResult,
    files: &[String],
    first: &[String],
    requirement_text: &dyn Fn(&str) -> String,
) -> Vec<Unit> {
    let mut red: Vec<&crate::accept::TestRun> = result.tests.iter().filter(|t| !t.green).collect();
    red.sort_by_key(|t| !first.contains(&t.id));
    if red.is_empty() {
        return Vec::new();
    }
    let size = red.len().div_ceil(MAX_UNITS);
    red.chunks(size)
        .map(|chunk| {
            let mut unit = Unit {
                tests: Vec::new(),
                requirements: Vec::new(),
                what: Vec::new(),
                files: Vec::new(),
                output: String::new(),
            };
            let mut evidence = String::new();
            for run in chunk {
                unit.tests.push(run.id.clone());
                for id in &run.requirements {
                    if !unit.requirements.contains(id) {
                        unit.requirements.push(id.clone());
                        evidence.push_str(&requirement_text(id));
                        evidence.push('\n');
                    }
                }
                if let Some(test) = suite.tests.iter().find(|t| t.id == run.id) {
                    unit.what.push(format!("{}: {}", test.id, test.what));
                    evidence.push_str(&test.source);
                    evidence.push('\n');
                }
                let tail = crate::judge::clip_tail(run.output.trim(), 600);
                evidence.push_str(&tail);
                evidence.push('\n');
                unit.output.push_str(&format!("{}: {tail}\n", run.id));
            }
            unit.files = files_named(&evidence, files);
            unit
        })
        .collect()
}

/// The code rule when Jev gives no answer: two units share a file unless
/// both name files and none of them in common. A unit that names no file
/// may touch any.
#[must_use]
pub fn code_shared(a: &Unit, b: &Unit) -> bool {
    a.files.is_empty() || b.files.is_empty() || a.files.iter().any(|f| b.files.contains(f))
}

/// The question whether units `a` and `b` of `units` would edit the same
/// file, with its state paths.
#[must_use]
pub fn shared_question(a: usize, b: usize) -> String {
    format!(
        "Two coding sessions will work at the same time, each in its own copy of the workspace \
         of the task in `task`, and their edits will then be merged into one workspace. One \
         session makes the red acceptance tests in `units[{a}]` pass and the other makes the red \
         tests in `units[{b}]` pass. Each unit lists the requirements its tests check, what each \
         test asserts, the end of each test's failing output, and `files`, the workspace files \
         named in that evidence. Would the two sessions need to edit the same file, or would one \
         session's change depend on the other's?"
    )
}

/// The key of the pair question for units `a` and `b`.
#[must_use]
pub fn pair_key(a: usize, b: usize) -> String {
    format!("shared_{a}_{b}")
}

/// Jev's state and one Noul per pair of units, asked together: the
/// questions read the same state and don't depend on each other's
/// answers, so one request answers every pair.
#[must_use]
pub fn independence(task: &str, units: &[Unit]) -> (Value, Questions) {
    let state = json!({
        "task": crate::judge::clip(task.trim(), 4_000),
        "units": units.iter().map(|u| json!({
            "tests": u.tests,
            "requirements": u.requirements,
            "what": u.what,
            "failing_output": crate::judge::clip(&u.output, 1_200),
            "files": u.files,
        })).collect::<Vec<_>>(),
    });
    let mut questions = Questions::new();
    for a in 0..units.len() {
        for b in a + 1..units.len() {
            questions = questions.with(
                pair_key(a, b),
                Noul::with_criteria(
                    shared_question(a, b).as_str(),
                    NoulCriteria::new()
                        .when_true(
                            "the two sessions would edit at least one file in common, or one \
                             fix depends on the other",
                        )
                        .when_false(
                            "each session's fix touches its own files and doesn't depend on the \
                             other's",
                        ),
                ),
            );
        }
    }
    (state, questions)
}

/// A group of units one session works on.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Lane {
    pub units: Vec<usize>,
    pub tests: Vec<String>,
    pub requirements: Vec<String>,
    pub files: Vec<String>,
}

/// Units packed into at most `cap` lanes: units joined by a shared pair
/// always share a lane (connected components), and when there are more
/// components than `cap`, the extra ones join the lane with the fewest
/// tests. `shared(a, b)` says whether units `a < b` share a file.
#[must_use]
pub fn lanes(units: &[Unit], shared: &dyn Fn(usize, usize) -> bool, cap: usize) -> Vec<Lane> {
    let n = units.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for a in 0..n {
        for b in a + 1..n {
            if shared(a, b) {
                let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
                if ra != rb {
                    parent[rb.max(ra)] = rb.min(ra);
                }
            }
        }
    }
    let mut components: Vec<Vec<usize>> = Vec::new();
    let mut seen: BTreeMap<usize, usize> = BTreeMap::new();
    for i in 0..n {
        let r = root(&mut parent, i);
        match seen.get(&r) {
            Some(&at) => components[at].push(i),
            None => {
                seen.insert(r, components.len());
                components.push(vec![i]);
            }
        }
    }
    let cap = cap.max(1);
    let mut packed: Vec<Vec<usize>> = Vec::new();
    for component in components {
        if packed.len() < cap {
            packed.push(component);
        } else {
            let tests =
                |lane: &Vec<usize>| lane.iter().map(|&u| units[u].tests.len()).sum::<usize>();
            let smallest = (0..packed.len())
                .min_by_key(|&i| tests(&packed[i]))
                .unwrap_or(0);
            packed[smallest].extend(component);
        }
    }
    packed
        .into_iter()
        .map(|mut members| {
            members.sort_unstable();
            let mut lane = Lane {
                units: members.clone(),
                tests: Vec::new(),
                requirements: Vec::new(),
                files: Vec::new(),
            };
            for u in members {
                lane.tests.extend(units[u].tests.iter().cloned());
                for id in &units[u].requirements {
                    if !lane.requirements.contains(id) {
                        lane.requirements.push(id.clone());
                    }
                }
                for file in &units[u].files {
                    if !lane.files.contains(file) {
                        lane.files.push(file.clone());
                    }
                }
            }
            lane
        })
        .collect()
}

/// Every file under `dir` a merge compares, by path relative to `dir`,
/// with its SHA-256 (a symbolic link by its target), skipping
/// [`UNMERGED`] directories and compiled Python files.
#[must_use]
pub fn tree(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let Ok(relative) = path.strip_prefix(dir) else {
                continue;
            };
            let relative = relative.to_string_lossy().into_owned();
            if kind.is_dir() {
                if !UNMERGED.contains(&name.as_str()) {
                    stack.push(path);
                }
            } else if kind.is_symlink() {
                let target = std::fs::read_link(&path).unwrap_or_default();
                out.insert(
                    relative,
                    crate::accept::sha256(format!("link:{}", target.display()).as_bytes()),
                );
            } else if !name.ends_with(".pyc")
                && let Ok(bytes) = std::fs::read(&path)
            {
                out.insert(relative, crate::accept::sha256(&bytes));
            }
        }
    }
    out
}

/// A lane whose changes weren't applied, and the files that clashed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conflict {
    pub lane: usize,
    pub files: Vec<String>,
}

/// What a merge did.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Merged {
    /// Lanes whose changes are in the workspace now, in order.
    pub applied: Vec<usize>,
    pub conflicts: Vec<Conflict>,
    /// Files the applied lanes changed, added, or removed.
    pub files: Vec<String>,
    /// Files two lanes changed that a line-level three-way merge joined.
    pub joined: Vec<String>,
}

/// `git merge-file` on the workspace's file, the base's, and the lane's:
/// the joined bytes, or `None` when the changes overlap or Git can't run.
fn merge_file(current: &Path, base: &Path, other: &Path) -> Option<Vec<u8>> {
    let output = std::process::Command::new("git")
        .args(["merge-file", "-p", "-q"])
        .arg(current)
        .arg(base)
        .arg(other)
        .output()
        .ok()?;
    (output.status.code() == Some(0)).then_some(output.stdout)
}

fn place(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    if to.symlink_metadata().is_ok() {
        std::fs::remove_file(to).map_err(|error| format!("{}: {error}", to.display()))?;
    }
    let kind = from
        .symlink_metadata()
        .map_err(|error| format!("{}: {error}", from.display()))?;
    if kind.file_type().is_symlink() {
        let target = std::fs::read_link(from).map_err(|error| error.to_string())?;
        std::os::unix::fs::symlink(target, to).map_err(|error| error.to_string())
    } else {
        std::fs::copy(from, to)
            .map(|_| ())
            .map_err(|error| format!("{}: {error}", from.display()))
    }
}

/// Merges each lane's copy into `real`, in order. A lane's change to a
/// file the workspace still has as `base` had it is taken as it is; a file
/// an earlier lane also changed is joined line by line (`git merge-file`),
/// and when that fails the lane conflicts. A conflicting lane changes
/// nothing: the first lane's work stands, and the caller requeues the
/// conflicting lane's tests.
#[must_use]
pub fn merge(real: &Path, base: &Path, copies: &[PathBuf]) -> Merged {
    enum Action {
        Take(String),
        Write(String, Vec<u8>),
    }
    let base_tree = tree(base);
    let mut out = Merged::default();
    for (lane, copy) in copies.iter().enumerate() {
        let lane_tree = tree(copy);
        let real_tree = tree(real);
        let paths: BTreeSet<&String> = base_tree.keys().chain(lane_tree.keys()).collect();
        let mut actions = Vec::new();
        let mut clashes = Vec::new();
        for path in paths {
            let (b, l) = (base_tree.get(path), lane_tree.get(path));
            if b == l {
                continue;
            }
            let r = real_tree.get(path);
            if r == l {
                continue;
            }
            if r == b {
                actions.push(Action::Take(path.clone()));
                continue;
            }
            let joined = match (b, l, r) {
                (Some(_), Some(_), Some(_)) => {
                    merge_file(&real.join(path), &base.join(path), &copy.join(path))
                }
                _ => None,
            };
            match joined {
                Some(bytes) => actions.push(Action::Write(path.clone(), bytes)),
                None => clashes.push(path.clone()),
            }
        }
        if !clashes.is_empty() {
            out.conflicts.push(Conflict {
                lane,
                files: clashes,
            });
            continue;
        }
        let mut failed = Vec::new();
        for action in actions {
            match action {
                Action::Take(path) => {
                    let result = if copy.join(&path).symlink_metadata().is_ok() {
                        place(&copy.join(&path), &real.join(&path))
                    } else {
                        std::fs::remove_file(real.join(&path)).map_err(|error| error.to_string())
                    };
                    match result {
                        Ok(()) => out.files.push(path),
                        Err(_) => failed.push(path),
                    }
                }
                Action::Write(path, bytes) => match std::fs::write(real.join(&path), bytes) {
                    Ok(()) => {
                        out.joined.push(path.clone());
                        out.files.push(path);
                    }
                    Err(_) => failed.push(path),
                },
            }
        }
        if failed.is_empty() {
            out.applied.push(lane);
        } else {
            out.conflicts.push(Conflict {
                lane,
                files: failed,
            });
        }
    }
    out
}

/// One stretch of work on the timeline: a Microluna session, or
/// `accept.define` as a whole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// `session 3`, `accept-writer-1-2`, or `accept.define`.
    pub label: String,
    /// `edit`, `writer`, `define`, or `guard`.
    pub kind: String,
    /// Tracks that were started together, such as `round 2` or `suite`.
    pub batch: String,
    /// What it worked on, such as `group 2 of 3: T3, T5`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// The directory it worked in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// Milliseconds from the dispatch's start.
    pub start_ms: u64,
    pub end_ms: u64,
    /// Model requests and input tokens, for a session.
    #[serde(default)]
    pub turns: usize,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub cached_tokens: u64,
    /// Turns that only read: every call a file read or a listing.
    #[serde(default)]
    pub read_turns: usize,
    #[serde(default)]
    pub cost_usd: f64,
}

impl Track {
    fn ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    fn session(&self) -> bool {
        self.kind == "edit" || self.kind == "writer"
    }
}

/// The loop's parallel summary, from its tracks and its merges:
///
/// - `session_ms`, every Microluna session's time added up, and
///   `concurrency`, that over the dispatch's wall time;
/// - `critical_path_ms`, the longest member of each top-level batch added
///   up: the time the same work needs when each batch runs fully in
///   parallel;
/// - `serial_estimate_ms`, the wall time had every batch run its members
///   one after another, and `saved_ms`, the difference from the wall;
/// - `suite_ms`, `accept.define`'s wall time, and
///   `suite_on_critical_path_ms`, how much of it the edit loop waited for
///   beyond the session that ran beside it;
/// - `peak`, the most sessions running at one time, and `overlaps`, every
///   pair of tracks that ran at the same time;
/// - `cached_share` and `read_turn_share` over every session;
/// - the merges, conflicts, and requeues.
#[must_use]
pub fn summary(tracks: &[Track], wall_ms: u64, merges: &[Value]) -> Value {
    let sessions: Vec<&Track> = tracks.iter().filter(|t| t.session()).collect();
    let session_ms: u64 = sessions.iter().map(|t| t.ms()).sum();
    let mut batches: Vec<(String, Vec<&Track>)> = Vec::new();
    for track in tracks {
        match batches.iter_mut().find(|(name, _)| *name == track.batch) {
            Some((_, members)) => members.push(track),
            None => batches.push((track.batch.clone(), vec![track])),
        }
    }
    let mut critical = 0u64;
    let mut saved = 0u64;
    let batch_rows: Vec<Value> = batches
        .iter()
        .map(|(name, members)| {
            let longest = members.iter().map(|t| t.ms()).max().unwrap_or(0);
            let sum: u64 = members.iter().map(|t| t.ms()).sum();
            // Writers run inside accept.define: their batch saves time
            // but isn't a step of its own on the critical path.
            let nested = members.iter().all(|t| t.kind == "writer");
            if !nested {
                critical += longest;
            }
            saved += sum - longest;
            let start = members.iter().map(|t| t.start_ms).min().unwrap_or(0);
            let end = members.iter().map(|t| t.end_ms).max().unwrap_or(0);
            json!({
                "batch": name,
                "members": members.iter().map(|t| t.label.clone()).collect::<Vec<_>>(),
                "start_ms": start,
                "end_ms": end,
                "longest_ms": longest,
                "sum_ms": sum,
                "parallel": members.len() > 1,
                "nested": nested,
                "cost_usd": members.iter().map(|t| t.cost_usd).sum::<f64>(),
            })
        })
        .collect();
    let mut overlaps = Vec::new();
    for (i, a) in tracks.iter().enumerate() {
        for b in &tracks[i + 1..] {
            let nested = (a.kind == "define" && b.kind == "writer")
                || (b.kind == "define" && a.kind == "writer");
            let ms = a
                .end_ms
                .min(b.end_ms)
                .saturating_sub(a.start_ms.max(b.start_ms));
            if ms > 0 && !nested {
                overlaps.push(json!({ "a": a.label, "b": b.label, "ms": ms }));
            }
        }
    }
    let mut edges: Vec<(u64, i32)> = Vec::new();
    for t in &sessions {
        edges.push((t.start_ms, 1));
        edges.push((t.end_ms, -1));
    }
    edges.sort_by_key(|&(at, delta)| (at, delta));
    let (mut now, mut peak) = (0i32, 0i32);
    for (_, delta) in edges {
        now += delta;
        peak = peak.max(now);
    }
    let define = tracks.iter().find(|t| t.kind == "define");
    let suite_ms = define.map(Track::ms);
    let beside = define.and_then(|d| {
        tracks
            .iter()
            .filter(|t| t.kind == "edit" && t.batch == d.batch)
            .map(Track::ms)
            .max()
    });
    let input: u64 = sessions.iter().map(|t| t.input_tokens).sum();
    let cached: u64 = sessions.iter().map(|t| t.cached_tokens).sum();
    let turns: usize = sessions.iter().map(|t| t.turns).sum();
    let read_turns: usize = sessions.iter().map(|t| t.read_turns).sum();
    let conflicts: usize = merges
        .iter()
        .map(|m| m["conflicts"].as_array().map_or(0, Vec::len))
        .sum();
    let ratio =
        |part: f64, whole: f64| (whole > 0.0).then(|| (part / whole * 1000.0).round() / 1000.0);
    json!({
        "wall_ms": wall_ms,
        "session_ms": session_ms,
        "concurrency": ratio(session_ms as f64, wall_ms as f64),
        "critical_path_ms": critical,
        "serial_estimate_ms": wall_ms + saved,
        "saved_ms": saved,
        "suite_ms": suite_ms,
        "suite_on_critical_path_ms": suite_ms.map(|s| s.saturating_sub(beside.unwrap_or(0))),
        "peak": peak,
        "sessions": sessions.len(),
        "turns": turns,
        "read_turn_share": ratio(read_turns as f64, turns as f64),
        "cached_share": ratio(cached as f64, input as f64),
        "merges": merges.len(),
        "conflicts": conflicts,
        "requeues": conflicts,
        "batches": batch_rows,
        "overlaps": overlaps,
        "tracks": tracks,
        "merge_rounds": merges,
    })
}

/// One line for a person: the wall, the concurrency, and what overlapped.
#[must_use]
pub fn headline(summary: &Value) -> String {
    let secs = |key: &str| {
        summary[key]
            .as_u64()
            .map_or("?".to_string(), |ms| format!("{:.1}s", ms as f64 / 1000.0))
    };
    format!(
        "{} sessions in {} wall ({} of session time, concurrency {}×, peak {}); suite {} ({} on the critical path); critical path {}; saved about {}; {} merges, {} conflicts; cached {}, read-only turns {}",
        summary["sessions"].as_u64().unwrap_or(0),
        secs("wall_ms"),
        secs("session_ms"),
        summary["concurrency"]
            .as_f64()
            .map_or("?".to_string(), |c| format!("{c:.2}")),
        summary["peak"].as_u64().unwrap_or(0),
        secs("suite_ms"),
        secs("suite_on_critical_path_ms"),
        secs("critical_path_ms"),
        secs("saved_ms"),
        summary["merges"].as_u64().unwrap_or(0),
        summary["conflicts"].as_u64().unwrap_or(0),
        summary["cached_share"]
            .as_f64()
            .map_or("?".to_string(), |c| format!("{:.0}%", c * 100.0)),
        summary["read_turn_share"]
            .as_f64()
            .map_or("?".to_string(), |c| format!("{:.0}%", c * 100.0)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(tests: &[&str], files: &[&str]) -> Unit {
        Unit {
            tests: tests.iter().map(|s| (*s).to_string()).collect(),
            requirements: vec!["R1".to_string()],
            what: Vec::new(),
            files: files.iter().map(|s| (*s).to_string()).collect(),
            output: String::new(),
        }
    }

    #[test]
    fn files_are_named_by_path_name_or_module() {
        let files: Vec<String> = [
            "drift_monitor/alert.py",
            "drift_monitor/distance.py",
            "data/ref.npy",
            "README",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        let text = "from drift_monitor.alert import Debouncer\ncat distance.py; python3 -c 'x'";
        assert_eq!(
            files_named(text, &files),
            ["drift_monitor/alert.py", "drift_monitor/distance.py"]
        );
        assert!(files_named("the READMEs", &files).is_empty());
    }

    #[test]
    fn shared_units_share_a_lane_and_the_rest_pack_under_the_cap() {
        let units = vec![
            unit(&["T1"], &["a.py"]),
            unit(&["T2"], &["b.py"]),
            unit(&["T3"], &["a.py", "c.py"]),
            unit(&["T4"], &["d.py"]),
            unit(&["T5"], &["e.py"]),
        ];
        let shared = |a: usize, b: usize| code_shared(&units[a], &units[b]);
        let packed = lanes(&units, &shared, 3);
        assert_eq!(packed.len(), 3);
        assert_eq!(packed[0].tests, ["T1", "T3"]);
        assert_eq!(packed[1].tests, ["T2", "T5"]);
        assert_eq!(packed[2].tests, ["T4"]);
        // A unit that names no file may touch any.
        assert!(code_shared(&unit(&["T6"], &[]), &units[0]));
    }

    #[test]
    fn one_noul_per_pair_reads_the_units_by_path() {
        let units = vec![
            unit(&["T1"], &["a.py"]),
            unit(&["T2"], &["b.py"]),
            unit(&["T3"], &[]),
        ];
        let (state, questions) = independence("Fix it.", &units);
        assert_eq!(state["units"].as_array().unwrap().len(), 3);
        let keys: Vec<&str> = questions.iter().map(|(key, _)| key).collect();
        assert_eq!(keys, ["shared_0_1", "shared_0_2", "shared_1_2"]);
        assert!(questions.validate().is_ok());
        assert!(shared_question(0, 2).contains("`units[0]`"));
        assert!(shared_question(0, 2).contains("`units[2]`"));
    }

    fn write(dir: &Path, path: &str, text: &str) {
        let at = dir.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }

    #[test]
    fn lanes_merge_cleanly_join_line_by_line_and_a_clash_is_left_out() {
        let root = tempfile::tempdir().unwrap();
        let (real, base) = (root.path().join("real"), root.path().join("base"));
        let text = "one\ntwo\nthree\nfour\nfive\nsix\nseven\n";
        write(&real, "a.py", "a = 1\n");
        write(&real, "shared.txt", text);
        write(&real, "gone.txt", "bye\n");
        crate::handoff::copy_tree(&real, &base).unwrap();
        let lanes: Vec<PathBuf> = (1..=3)
            .map(|i| root.path().join(format!("lane{i}")))
            .collect();
        for lane in &lanes {
            crate::handoff::copy_tree(&real, lane).unwrap();
        }
        // Lane 1 edits a.py, the top of shared.txt, adds a file, and
        // deletes one; its cache files don't count.
        write(&lanes[0], "a.py", "a = 2\n");
        write(&lanes[0], "shared.txt", &text.replace("one", "ONE"));
        write(&lanes[0], "new/b.py", "b = 1\n");
        write(&lanes[0], "__pycache__/a.cpython-312.pyc", "x");
        std::fs::remove_file(lanes[0].join("gone.txt")).unwrap();
        // Lane 2 edits the bottom of shared.txt: joined line by line.
        write(&lanes[1], "shared.txt", &text.replace("seven", "SEVEN"));
        write(&lanes[1], "__pycache__/a.cpython-312.pyc", "y");
        // Lane 3 edits a.py differently: a clash, so nothing of it lands.
        write(&lanes[2], "a.py", "a = 3\n");
        write(&lanes[2], "c.py", "c = 1\n");
        let merged = merge(&real, &base, &lanes);
        assert_eq!(merged.applied, [0, 1]);
        assert_eq!(
            merged.conflicts,
            [Conflict {
                lane: 2,
                files: vec!["a.py".to_string()]
            }]
        );
        assert_eq!(merged.joined, ["shared.txt"]);
        let read = |p: &str| std::fs::read_to_string(real.join(p)).ok();
        assert_eq!(read("a.py").as_deref(), Some("a = 2\n"));
        assert_eq!(read("new/b.py").as_deref(), Some("b = 1\n"));
        assert_eq!(read("gone.txt"), None);
        assert_eq!(read("c.py"), None, "a conflicting lane changes nothing");
        assert_eq!(
            read("shared.txt").as_deref(),
            Some("ONE\ntwo\nthree\nfour\nfive\nsix\nSEVEN\n")
        );
        assert!(!real.join("__pycache__").exists());
    }

    fn track(label: &str, kind: &str, batch: &str, start: u64, end: u64) -> Track {
        Track {
            label: label.to_string(),
            kind: kind.to_string(),
            batch: batch.to_string(),
            group: None,
            workspace: None,
            start_ms: start,
            end_ms: end,
            turns: 10,
            input_tokens: 1_000,
            cached_tokens: 500,
            read_turns: 4,
            cost_usd: 0.01,
        }
    }

    #[test]
    fn the_summary_counts_overlap_the_critical_path_and_the_saving() {
        let tracks = vec![
            track("accept.define", "define", "suite", 0, 60_000),
            track("accept-writer-1-1", "writer", "writers 1", 1_000, 40_000),
            track("accept-writer-1-2", "writer", "writers 1", 1_000, 50_000),
            track("session 1", "edit", "suite", 0, 45_000),
            track("session 2", "edit", "round 2", 61_000, 90_000),
            track("session 3", "edit", "round 2", 61_000, 81_000),
        ];
        let merges = vec![json!({ "conflicts": [{ "lane": 1 }] })];
        let s = summary(&tracks, 100_000, &merges);
        assert_eq!(
            s["session_ms"],
            json!(39_000 + 49_000 + 45_000 + 29_000 + 20_000)
        );
        assert_eq!(s["peak"], json!(3));
        // suite: max(define, session 1); round 2: session 2.
        assert_eq!(s["critical_path_ms"], json!(60_000 + 29_000));
        // suite saves session 1, the writers save writer 1, round 2
        // saves session 3.
        assert_eq!(s["saved_ms"], json!(45_000 + 39_000 + 20_000));
        assert_eq!(s["suite_ms"], json!(60_000));
        assert_eq!(s["suite_on_critical_path_ms"], json!(15_000));
        assert_eq!(s["cached_share"], json!(0.5));
        assert_eq!(s["read_turn_share"], json!(0.4));
        assert_eq!(s["conflicts"], json!(1));
        let pairs: Vec<(String, String)> = s["overlaps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| {
                (
                    o["a"].as_str().unwrap().to_string(),
                    o["b"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert!(pairs.contains(&("session 2".to_string(), "session 3".to_string())));
        assert!(pairs.contains(&("accept.define".to_string(), "session 1".to_string())));
        assert!(
            !pairs
                .iter()
                .any(|(a, b)| a == "accept.define" && b.starts_with("accept-writer"))
        );
        assert!(headline(&s).contains("concurrency 1.82×"));
    }
}
