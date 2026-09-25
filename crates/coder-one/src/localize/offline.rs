//! Failure localization on retained Microluna sessions (issue #9658),
//! under the protocol in
//! `bench/terminal-bench/experiments/2026-09-25-failure-localization/`.
//!
//! For every failing command in a retained session, code asks what
//! `evidence.error_context` would have found in its output, whether the
//! session's next edit touched that region, and how many later turns only
//! re-read the files the error named. It runs nothing and asks no model.
//! The next edit touching the region is an association, not a causal
//! claim: the session read the same output the component would have
//! printed from.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::context;
use super::trace::{self, Call, Log};
use super::{mismatch, parse, timing};

/// The row schema.
pub const ROW_SCHEMA: &str = "openagents.coder_one.localize_offline_row.v1";

/// The window the measurement uses: the component's default.
pub const WINDOW: usize = context::WINDOW;

/// What the next edit after a failing command did to the localized
/// region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Touch {
    /// It changed a line inside the window.
    Region,
    /// It changed the file, outside the window.
    FileOnly,
    /// It changed other files only.
    OtherFile,
    /// The session made no edit after the failure.
    NoEdit,
    /// The file's text before the edit isn't known, so the lines it
    /// changed aren't.
    Undetermined,
}

/// One failing command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub schema: String,
    pub trial: String,
    pub task: String,
    /// `microluna-<d>-<n>`.
    pub session: String,
    pub call: usize,
    pub turn: usize,
    pub command: String,
    pub exit: Option<i64>,
    pub timed_out: bool,
    /// References the parser found.
    pub references: usize,
    /// The rules that found them.
    pub rules: Vec<String>,
    /// The first reference that resolved to a known file, and its line.
    pub file: Option<String>,
    pub line: Option<u64>,
    /// The last failing command before the next edit.
    pub last_before_edit: bool,
    pub touch: Option<Touch>,
    /// The chance that the same edit touches a window around a uniformly
    /// random line of the file, when the file's lines are known.
    pub chance: Option<f64>,
    /// Later read-only turns, before the next edit, that read the file.
    pub reread_turns: usize,
    /// Of those, turns that read the named line itself.
    pub reread_line_turns: usize,
    /// The mismatch parser found a case, and its format.
    pub mismatch: Option<String>,
    pub mismatch_stages: usize,
    /// The profiler `evidence.phase_timing` would pick, for a timed-out
    /// command.
    pub profiler: Option<timing::Profiler>,
}

/// One trial's sessions, in session order, with the files read.
struct Trial {
    id: String,
    task: String,
    sessions: Vec<(String, Log)>,
    sources: Vec<(String, String)>,
}

fn read_trials(roots: &[PathBuf], excluded: &[String]) -> (Vec<Trial>, usize) {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut skipped = 0;
    for root in roots {
        for artifacts in crate::component::stall::artifact_dirs(root) {
            let Some((_, id)) = crate::component::stall::trial_of(&artifacts) else {
                continue;
            };
            let task = id.split("__").next().unwrap_or_default().to_string();
            if excluded.contains(&task) {
                skipped += 1;
                continue;
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            let mut files: Vec<((u32, u32), PathBuf)> = std::fs::read_dir(&artifacts)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|f| {
                    Some((
                        crate::component::stall::session_number(&f.file_name().to_string_lossy())?,
                        f.path(),
                    ))
                })
                .collect();
            files.sort();
            let mut sessions = Vec::new();
            let mut sources = Vec::new();
            for ((d, n), path) in files {
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                sources.push((path.display().to_string(), crate::accept::sha256(&bytes)));
                sessions.push((
                    format!("microluna-{d}-{n}"),
                    trace::parse(&String::from_utf8_lossy(&bytes)),
                ));
            }
            out.push(Trial {
                id,
                task,
                sessions,
                sources,
            });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    (out, skipped)
}

/// `path` relative to the workspace `root`, without `./`.
fn relative(path: &str, root: &str) -> String {
    let root = root.trim_end_matches('/');
    let path = if root.is_empty() {
        path
    } else {
        path.strip_prefix(&format!("{root}/")).unwrap_or(path)
    };
    let path = path.strip_prefix("/app/").unwrap_or(path);
    path.trim_start_matches("./").to_string()
}

/// The files a patch names.
fn patch_paths(text: &str) -> Vec<String> {
    microluna::patch::parse(text)
        .map(|hunks| {
            hunks
                .iter()
                .map(|h| match h {
                    microluna::patch::Hunk::Add { path, .. }
                    | microluna::patch::Hunk::Delete { path }
                    | microluna::patch::Hunk::Update { path, .. } => path.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Every file the trial's sessions show: in a brief, or read, written, or
/// patched.
fn known_files(trial: &Trial) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (_, log) in &trial.sessions {
        for (path, _) in &log.brief {
            out.insert(relative(path, &log.root));
        }
        for (_, call) in &log.calls {
            match call {
                Call::Read { path, .. } | Call::Write { path, .. } => {
                    out.insert(relative(path, &log.root));
                }
                Call::Patch { text, .. } => {
                    for path in patch_paths(text) {
                        out.insert(relative(&path, &log.root));
                    }
                }
                _ => {}
            }
        }
    }
    out.retain(|p| !p.is_empty() && !p.starts_with('/'));
    out
}

/// Spans of old lines, 1-based and inclusive, that an edit from `old` to
/// `new` changed; an insertion between lines k and k+1 touches both.
#[must_use]
pub fn changed_spans(old: &str, new: &str) -> Vec<(usize, usize)> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let a_mid = &a[prefix..a.len() - suffix];
    let b_mid = &b[prefix..b.len() - suffix];
    if a_mid.is_empty() && b_mid.is_empty() {
        return Vec::new();
    }
    let span = |first: usize, last: usize| (first.max(1), last.max(first.max(1)));
    // A line diff of the middle when it's small enough, else its span.
    if a_mid.len().saturating_mul(b_mid.len()) > 4_000_000 || a_mid.is_empty() || b_mid.is_empty() {
        return if a_mid.is_empty() {
            vec![span(prefix, prefix + 1)]
        } else {
            vec![span(prefix + 1, prefix + a_mid.len())]
        };
    }
    let (n, m) = (a_mid.len(), b_mid.len());
    let mut lcs = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i * (m + 1) + j] = if a_mid[i] == b_mid[j] {
                lcs[(i + 1) * (m + 1) + j + 1] + 1
            } else {
                lcs[(i + 1) * (m + 1) + j].max(lcs[i * (m + 1) + j + 1])
            };
        }
    }
    // Walk the table in runs of changes. A run that removes lines touches
    // them; a run that only inserts before old line i touches its two
    // neighbors, lines i and i+1 of the file.
    let mut touched: BTreeSet<usize> = BTreeSet::new();
    let (mut i, mut j) = (0, 0);
    let mut removed: Vec<usize> = Vec::new();
    let mut inserted_at: Option<usize> = None;
    let mut close = |removed: &mut Vec<usize>, inserted_at: &mut Option<usize>| {
        if removed.is_empty() {
            if let Some(at) = inserted_at.take() {
                touched.insert(prefix + at);
                touched.insert(prefix + at + 1);
            }
        } else {
            touched.extend(removed.drain(..).map(|k| prefix + k + 1));
            *inserted_at = None;
        }
    };
    while i < n || j < m {
        if i < n && j < m && a_mid[i] == b_mid[j] {
            close(&mut removed, &mut inserted_at);
            i += 1;
            j += 1;
        } else if i < n && (j == m || lcs[(i + 1) * (m + 1) + j] >= lcs[i * (m + 1) + j + 1]) {
            removed.push(i);
            i += 1;
        } else {
            inserted_at.get_or_insert(i);
            j += 1;
        }
    }
    close(&mut removed, &mut inserted_at);
    let old_len = a.len();
    touched.retain(|l| *l <= old_len.max(1));
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for line in touched.into_iter().filter(|l| *l >= 1) {
        match spans.last_mut() {
            Some((_, last)) if *last + 1 >= line => *last = line,
            _ => spans.push((line, line)),
        }
    }
    spans
}

fn touches(spans: &[(usize, usize)], line: usize, window: usize) -> bool {
    let (first, last) = (line.saturating_sub(window).max(1), line + window);
    spans.iter().any(|(a, b)| *a <= last && first <= *b)
}

/// The chance that `spans` touch a window around a uniformly random line
/// of a `lines`-line file.
fn chance(spans: &[(usize, usize)], lines: usize, window: usize) -> f64 {
    if lines == 0 {
        return 0.0;
    }
    let hit = (1..=lines).filter(|c| touches(spans, *c, window)).count();
    hit as f64 / lines as f64
}

/// What one completed edit did, per file, against the files known before
/// it: `Some(spans)` when the old text was known, `None` when it wasn't.
type EditEffect = BTreeMap<String, Option<(Vec<(usize, usize)>, usize)>>;

/// The session's files as far as its log shows them, updated call by call.
#[derive(Default)]
struct Files {
    text: BTreeMap<String, String>,
}

impl Files {
    fn start(log: &Log) -> Files {
        Files {
            text: log
                .brief
                .iter()
                .map(|(p, t)| (relative(p, &log.root), t.clone()))
                .collect(),
        }
    }

    /// Applies `call`, and returns its effect when it's a completed edit.
    fn apply(&mut self, call: &Call, root: &str) -> Option<EditEffect> {
        match call {
            Call::Read {
                path,
                lines,
                to_end,
            } => {
                if *to_end && lines.first().is_some_and(|(n, _)| *n == 1) {
                    let text: Vec<&str> = lines.iter().map(|(_, l)| l.as_str()).collect();
                    self.text.insert(relative(path, root), text.join("\n"));
                }
                None
            }
            Call::Write {
                path,
                contents,
                ok: true,
            } => {
                let path = relative(path, root);
                let effect = self
                    .text
                    .get(&path)
                    .map(|old| (changed_spans(old, contents), old.lines().count()));
                self.text.insert(path.clone(), contents.clone());
                Some(BTreeMap::from([(path, effect)]))
            }
            Call::Patch { text, ok: true } => {
                let mut effect = EditEffect::new();
                let Ok(hunks) = microluna::patch::parse(text) else {
                    return Some(effect);
                };
                for hunk in hunks {
                    match hunk {
                        microluna::patch::Hunk::Add { path, contents } => {
                            let path = relative(&path, root);
                            let old = self.text.get(&path).cloned().unwrap_or_default();
                            effect.insert(
                                path.clone(),
                                Some((changed_spans(&old, &contents), old.lines().count())),
                            );
                            self.text.insert(path, contents);
                        }
                        microluna::patch::Hunk::Delete { path } => {
                            let path = relative(&path, root);
                            let old = self.text.remove(&path);
                            effect.insert(
                                path,
                                old.map(|o| {
                                    (vec![(1, o.lines().count().max(1))], o.lines().count())
                                }),
                            );
                        }
                        microluna::patch::Hunk::Update {
                            path,
                            move_to,
                            chunks,
                        } => {
                            let path = relative(&path, root);
                            let Some(old) = self.text.get(&path).cloned() else {
                                effect.insert(path, None);
                                continue;
                            };
                            // Chunk by chunk, so each one's span is exact.
                            let mut current = old.clone();
                            let mut spans = Vec::new();
                            let mut shift: isize = 0;
                            let mut failed = false;
                            for chunk in &chunks {
                                let Ok(next) =
                                    microluna::patch::apply(&current, std::slice::from_ref(chunk))
                                else {
                                    failed = true;
                                    break;
                                };
                                for (a, b) in changed_spans(&current, &next) {
                                    let map = |l: usize| {
                                        usize::try_from(l as isize - shift).unwrap_or(1).max(1)
                                    };
                                    spans.push((map(a), map(b)));
                                }
                                shift += next.lines().count() as isize
                                    - current.lines().count() as isize;
                                current = next;
                            }
                            if failed {
                                self.text.remove(&path);
                                effect.insert(path, None);
                                continue;
                            }
                            effect.insert(path.clone(), Some((spans, old.lines().count())));
                            match move_to {
                                Some(to) => {
                                    self.text.remove(&path);
                                    self.text.insert(relative(&to, root), current);
                                }
                                None => {
                                    self.text.insert(path, current);
                                }
                            }
                        }
                    }
                }
                Some(effect)
            }
            _ => None,
        }
    }
}

/// Whether the read-only turn's `calls` read `file`, and whether one of
/// them showed `line`.
fn rereads(calls: &[&Call], file: &str, line: u64, root: &str) -> (bool, bool) {
    let name = file.rsplit('/').next().unwrap_or(file);
    let mut reads = false;
    let mut covers = false;
    let line = usize::try_from(line).unwrap_or(usize::MAX);
    for call in calls {
        match call {
            Call::Read { path, lines, .. } if relative(path, root) == file => {
                reads = true;
                covers |= lines.iter().any(|(n, _)| *n == line);
            }
            Call::Command { command, .. } if command.contains(name) => {
                reads = true;
                let ranges = sed_ranges(command);
                let whole = command
                    .split_whitespace()
                    .any(|w| matches!(w, "cat" | "nl"));
                covers |= whole || ranges.iter().any(|(a, b)| *a <= line && line <= *b);
            }
            _ => {}
        }
    }
    (reads, covers)
}

/// `sed -n 'a,bp'` ranges in a command.
fn sed_ranges(command: &str) -> Vec<(usize, usize)> {
    static SED: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let pattern = SED.get_or_init(|| {
        regex::Regex::new(r"(\d+),(\d+)p").expect("the sed range pattern compiles")
    });
    pattern
        .captures_iter(command)
        .filter_map(|c| Some((c[1].parse().ok()?, c[2].parse().ok()?)))
        .collect()
}

/// The rows of one session.
fn session_rows(trial: &Trial, known: &BTreeSet<String>, name: &str, log: &Log) -> Vec<Row> {
    let root = log.root.as_str();
    // Each call's effect, computed in order.
    let mut files = Files::start(log);
    let effects: Vec<Option<EditEffect>> = log
        .calls
        .iter()
        .map(|(_, call)| files.apply(call, root))
        .collect();
    let mut rows = Vec::new();
    for (index, (turn, call)) in log.calls.iter().enumerate() {
        let Some(failure) = call.failure() else {
            continue;
        };
        let locations = parse::parse(&failure.output);
        let located = context::located(std::slice::from_ref(&failure), known, root);
        let first = located.first();
        let next_edit = (index + 1..log.calls.len()).find(|i| log.calls[*i].1.edit());
        let last_before_edit = !(index + 1..next_edit.unwrap_or(log.calls.len()))
            .any(|i| log.calls[i].1.failure().is_some());
        let (touch, chance_of) = match first {
            None => (None, None),
            Some((file, location, _)) => match next_edit {
                None => (Some(Touch::NoEdit), None),
                Some(at) => {
                    let effect = effects[at].clone().unwrap_or_default();
                    match effect.get(file) {
                        None => (Some(Touch::OtherFile), None),
                        Some(None) => (Some(Touch::Undetermined), None),
                        Some(Some((spans, lines))) => {
                            let line = usize::try_from(location.line).unwrap_or(usize::MAX);
                            let hit = touches(spans, line, WINDOW);
                            (
                                Some(if hit { Touch::Region } else { Touch::FileOnly }),
                                Some(chance(spans, *lines, WINDOW)),
                            )
                        }
                    }
                }
            },
        };
        // Read-only turns after this one, before the next edit, that
        // re-read the named file.
        let (mut reread_turns, mut reread_line_turns) = (0, 0);
        if let Some((file, location, _)) = first {
            let end = next_edit.unwrap_or(log.calls.len());
            let mut by_turn: BTreeMap<usize, Vec<&Call>> = BTreeMap::new();
            for (t, c) in &log.calls[index + 1..end] {
                if *t > *turn {
                    by_turn.entry(*t).or_default().push(c);
                }
            }
            for calls in by_turn.values() {
                let read_only = calls.iter().all(|c| {
                    matches!(c, Call::Read { .. })
                        || matches!(
                            c,
                            Call::Command {
                                reads_only: true,
                                ..
                            }
                        )
                });
                if !read_only {
                    continue;
                }
                let (reads, covers) = rereads(calls, file, location.line, root);
                reread_turns += usize::from(reads);
                reread_line_turns += usize::from(reads && covers);
            }
        }
        let case = mismatch::first_case(&failure.output);
        rows.push(Row {
            schema: ROW_SCHEMA.to_string(),
            trial: trial.id.clone(),
            task: trial.task.clone(),
            session: name.to_string(),
            call: index,
            turn: *turn,
            command: crate::judge::clip(&failure.command, 300),
            exit: failure.exit,
            timed_out: failure.timed_out,
            references: locations.len(),
            rules: locations
                .iter()
                .map(|l| l.rule.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            file: first.map(|(f, _, _)| f.clone()),
            line: first.map(|(_, l, _)| l.line),
            last_before_edit,
            touch,
            chance: chance_of,
            reread_turns,
            reread_line_turns,
            mismatch: case.as_ref().map(|c| c.format.clone()),
            mismatch_stages: case.as_ref().map_or(0, |c| c.stages.len()),
            profiler: failure
                .timed_out
                .then(|| timing::plan(&failure.command, 60).0),
        });
    }
    rows
}

/// Turn counts per session, for the denominators.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Totals {
    pub trials: usize,
    pub sessions: usize,
    pub turns: usize,
    /// Turns whose every call only read.
    pub read_only_turns: usize,
    pub commands: usize,
    pub failing_commands: usize,
    pub skipped_excluded: usize,
}

/// Every row under `roots`, leaving out `excluded` tasks, with the totals
/// and the files read.
#[must_use]
pub fn replay(roots: &[PathBuf], excluded: &[String]) -> (Vec<Row>, Totals, Value) {
    let (trials, skipped) = read_trials(roots, excluded);
    let mut rows = Vec::new();
    let mut totals = Totals {
        trials: trials.len(),
        skipped_excluded: skipped,
        ..Totals::default()
    };
    let mut sources = Vec::new();
    for trial in &trials {
        let known = known_files(trial);
        for (name, log) in &trial.sessions {
            totals.sessions += 1;
            totals.turns += log.turns;
            let mut by_turn: BTreeMap<usize, Vec<&Call>> = BTreeMap::new();
            for (t, c) in &log.calls {
                by_turn.entry(*t).or_default().push(c);
                if let Call::Command { .. } = c {
                    totals.commands += 1;
                    totals.failing_commands += usize::from(c.failure().is_some());
                }
            }
            totals.read_only_turns += by_turn
                .values()
                .filter(|calls| {
                    calls.iter().all(|c| {
                        matches!(c, Call::Read { .. })
                            || matches!(
                                c,
                                Call::Command {
                                    reads_only: true,
                                    ..
                                }
                            )
                    })
                })
                .count();
            rows.extend(session_rows(trial, &known, name, log));
        }
        sources.push(json!({
            "trial": trial.id,
            "task": trial.task,
            "files": trial.sources.iter().map(|(p, h)| json!({"path": p, "sha256": h})).collect::<Vec<_>>(),
        }));
    }
    (
        rows,
        totals,
        json!({ "excluded_tasks": excluded, "trials": sources }),
    )
}

/// The Wilson 95% interval of `k` of `n`.
#[must_use]
pub fn wilson(k: usize, n: usize) -> Option<(f64, f64)> {
    if n == 0 {
        return None;
    }
    let (k, n, z) = (k as f64, n as f64, 1.96_f64);
    let p = k / n;
    let center = (p + z * z / (2.0 * n)) / (1.0 + z * z / n);
    let half = z * ((p * (1.0 - p) + z * z / (4.0 * n)) / n).sqrt() / (1.0 + z * z / n);
    Some(((center - half).max(0.0), (center + half).min(1.0)))
}

fn rate(k: usize, n: usize) -> Value {
    json!({
        "k": k,
        "n": n,
        "rate": (n > 0).then(|| k as f64 / n as f64),
        "wilson95": wilson(k, n).map(|(a, b)| [a, b]),
    })
}

fn touch_block(rows: &[&Row]) -> Value {
    let with_edit: Vec<&&Row> = rows
        .iter()
        .filter(|r| r.touch.is_some() && r.touch != Some(Touch::NoEdit))
        .collect();
    let count = |t: Touch| rows.iter().filter(|r| r.touch == Some(t)).count();
    let determined: Vec<&&Row> = with_edit
        .iter()
        .copied()
        .filter(|r| r.touch != Some(Touch::Undetermined))
        .collect();
    let same_file: Vec<&&Row> = determined
        .iter()
        .copied()
        .filter(|r| matches!(r.touch, Some(Touch::Region | Touch::FileOnly)))
        .collect();
    let chance: Vec<f64> = same_file.iter().filter_map(|r| r.chance).collect();
    json!({
        "localized": rows.len(),
        "region": count(Touch::Region),
        "file_only": count(Touch::FileOnly),
        "other_file": count(Touch::OtherFile),
        "no_edit": count(Touch::NoEdit),
        "undetermined": count(Touch::Undetermined),
        "region_of_determined_edits": rate(count(Touch::Region), determined.len()),
        "region_of_same_file_edits": rate(count(Touch::Region), same_file.len()),
        "chance_of_same_file_edits": (!chance.is_empty()).then(|| chance.iter().sum::<f64>() / chance.len() as f64),
    })
}

/// The summary of `rows`.
#[must_use]
pub fn summary(rows: &[Row], totals: &Totals, sources: &Value) -> Value {
    let failing: Vec<&Row> = rows.iter().collect();
    let parsed = failing.iter().filter(|r| r.references > 0).count();
    let resolved: Vec<&Row> = failing
        .iter()
        .copied()
        .filter(|r| r.file.is_some())
        .collect();
    let last: Vec<&Row> = resolved
        .iter()
        .copied()
        .filter(|r| r.last_before_edit)
        .collect();
    let reread: usize = resolved.iter().map(|r| r.reread_turns).sum();
    let reread_line: usize = resolved.iter().map(|r| r.reread_line_turns).sum();
    let mut by_task: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for r in &failing {
        let e = by_task.entry(r.task.clone()).or_default();
        e.0 += 1;
        e.1 += usize::from(r.references > 0);
        e.2 += usize::from(r.file.is_some());
    }
    let mut by_rule: BTreeMap<String, usize> = BTreeMap::new();
    for r in &failing {
        for rule in &r.rules {
            *by_rule.entry(rule.clone()).or_default() += 1;
        }
    }
    let mismatches = failing.iter().filter(|r| r.mismatch.is_some()).count();
    let mut mismatch_formats: BTreeMap<String, usize> = BTreeMap::new();
    for r in &failing {
        if let Some(f) = &r.mismatch {
            *mismatch_formats.entry(f.clone()).or_default() += 1;
        }
    }
    let timed_out: Vec<&Row> = failing.iter().copied().filter(|r| r.timed_out).collect();
    let mut profilers: BTreeMap<String, usize> = BTreeMap::new();
    for r in &timed_out {
        if let Some(p) = r.profiler {
            *profilers
                .entry(
                    serde_json::to_value(p)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default(),
                )
                .or_default() += 1;
        }
    }
    json!({
        "schema": "openagents.coder_one.localize_offline_summary.v1",
        "window": WINDOW,
        "totals": totals,
        "parsable": rate(parsed, failing.len()),
        "resolved": rate(resolved.len(), failing.len()),
        "resolved_of_parsable": rate(resolved.len(), parsed),
        "by_rule": by_rule,
        "by_task": by_task.iter().map(|(t, (n, p, r))| (t.clone(), json!({"failing": n, "parsable": p, "resolved": r}))).collect::<BTreeMap<_, _>>(),
        "next_edit_every_failure": touch_block(&resolved),
        "next_edit_last_failure_before_edit": touch_block(&last),
        "rereads": {
            "turns": reread,
            "turns_reading_the_named_line": reread_line,
            "share_of_all_turns": (totals.turns > 0).then(|| reread as f64 / totals.turns as f64),
            "share_of_read_only_turns": (totals.read_only_turns > 0).then(|| reread as f64 / totals.read_only_turns as f64),
            "localized_failures_followed_by_a_reread": rate(resolved.iter().filter(|r| r.reread_turns > 0).count(), resolved.len()),
        },
        "mismatch": {
            "cases": rate(mismatches, failing.len()),
            "formats": mismatch_formats,
            "with_stages": failing.iter().filter(|r| r.mismatch_stages > 0).count(),
        },
        "timing": {
            "timed_out": rate(timed_out.len(), failing.len()),
            "profilers": profilers,
        },
        "sources": sources["trials"].as_array().map_or(0, Vec::len),
    })
}

/// Writes `rows.jsonl`, `summary.json`, and `sources.json` to `out`.
///
/// # Errors
///
/// Returns a message when a file can't be written.
pub fn write(out: &Path, rows: &[Row], summary: &Value, sources: &Value) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut lines = String::new();
    for row in rows {
        lines.push_str(&serde_json::to_string(row).map_err(|e| e.to_string())?);
        lines.push('\n');
    }
    crate::record::write_atomic(&out.join("rows.jsonl"), lines.as_bytes())?;
    for (name, value) in [("summary.json", summary), ("sources.json", sources)] {
        let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
        crate::record::write_atomic(&out.join(name), format!("{text}\n").as_bytes())?;
    }
    Ok(())
}
