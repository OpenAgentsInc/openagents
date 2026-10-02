//! Judging unknown folders (phase 3). When a disk cleanup run falls short,
//! the largest folders no class covers are each shown to Jev as a
//! code-built state (path, size, age, top entries, marker files, and how
//! many processes use it now) with a Noul, "is this output a program will
//! regenerate, or a download cache, holding nothing a person made?", and
//! a Choice of kind. A yes at or above `background.cache_dir` (0.9) and a
//! disposable kind never deletes anything: it is a proposal the person
//! confirms or declines. A confirmed folder becomes a plain entry of the
//! rule (`classes.judged`, class 7), so later runs need no model, and its
//! contents go to the background trash for 24 hours before they are gone.
//!
//! Code decides what is never asked about: Git checkouts, anything on the
//! deny list or in a privacy-protected place, links, other volumes, and
//! anything that holds or sits inside a folder a class already covers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::{Judge, Question, Setting};
use crate::paths::{self, Layout, bytes, real_dir, show};
use crate::plan::Env;
use crate::rule::{self, Action, Class, Judged, Rule};
use crate::store::{self, write_atomic};

/// `background.cache_dir`: how sure Jev must be that a folder is a
/// disposable cache before it is proposed. Unmeasured.
pub const CACHE_DIR: Setting = Setting::new("background.cache_dir", 0.9);

/// The question.
pub const QUESTION: &str = "Is this directory output a program will regenerate, or a download \
cache, holding nothing a person made?";

/// The kinds a folder may be, as Jev chooses among them.
pub const ALL_KINDS: [(&str, &str); 6] = [
    (
        "build_output",
        "Output a build tool writes and rebuilds (compiled objects, bundles).",
    ),
    (
        "package_cache",
        "Downloaded packages or dependencies a package manager fetches again.",
    ),
    (
        "app_cache",
        "A program's cache it refills on its own (thumbnails, downloads, indexes).",
    ),
    (
        "user_data",
        "Things a person made or collected: documents, media, datasets, settings, keys.",
    ),
    ("source", "Source code or a project someone works on."),
    ("unknown", "Cannot tell from what is shown."),
];

/// The kinds that may be proposed.
pub const KINDS: [&str; 3] = ["build_output", "package_cache", "app_cache"];

/// Where unknown folders are looked for: the children of these.
pub const SURVEY: [&str; 3] = ["~/.openagents", "~/work", "~/.cache"];

/// Folders smaller than this are not worth a judgment.
pub const MIN_BYTES: u64 = rule::GB;

/// At most this many of the largest folders are judged in one look.
pub const LARGEST: usize = 5;

/// A folder already judged is not asked about again for this long.
pub const AGAIN_SECS: u64 = 30 * 86_400;

/// One folder no class covers, as code sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unknown {
    pub path: PathBuf,
    pub bytes: u64,
    /// Seconds since anything at its top changed.
    pub age_secs: u64,
    /// Its largest entries and their sizes.
    pub top: Vec<(String, u64)>,
    /// Marker files and folders at its top.
    pub markers: Vec<String>,
    /// Open files and working folders inside it now.
    pub users: usize,
}

/// The marker names a state reports when present at a folder's top.
pub const MARKERS: [&str; 14] = [
    "CACHEDIR.TAG",
    ".git",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "node_modules",
    "target",
    "build",
    "dist",
    ".DS_Store",
    "README.md",
    "index",
    "cache",
];

impl Unknown {
    /// The state Jev reads: code-built facts, never a transcript.
    #[must_use]
    pub fn state(&self, home: &Path) -> String {
        let top: Vec<String> = self
            .top
            .iter()
            .map(|(name, size)| format!("{name} ({})", bytes(*size)))
            .collect();
        format!(
            "Path: {}\nSize: {}\nLast changed: {} days ago\nLargest entries: {}\nMarkers: {}\nProcesses using it now: {}",
            show(&self.path, home),
            bytes(self.bytes),
            self.age_secs / 86_400,
            if top.is_empty() {
                "none".into()
            } else {
                top.join(", ")
            },
            if self.markers.is_empty() {
                "none".into()
            } else {
                self.markers.join(", ")
            },
            self.users
        )
    }
}

/// How a proposal stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Waiting for the person.
    Proposed,
    Confirmed,
    /// The person said no: never asked about again.
    Declined,
    /// Jev did not judge it a cache; asked again after [`AGAIN_SECS`].
    NotCache,
}

/// One judged or proposed folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    /// As a rule names it (`~/…`).
    pub path: String,
    pub bytes: u64,
    pub kind: String,
    /// The Noul's probability; `None` for a plugin's proposal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind_probability: Option<f64>,
    pub setting: String,
    pub asked: u64,
    pub status: Status,
    /// The rule that fell short, which a confirmation edits.
    pub rule: String,
    /// The plugin that proposed it, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
}

/// The proposals file, by path.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Proposals {
    #[serde(default)]
    pub folders: BTreeMap<String, Proposal>,
}

impl Proposals {
    #[must_use]
    pub fn load(layout: &Layout) -> Self {
        std::fs::read(layout.proposals())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// # Errors
    /// The file cannot be written.
    pub fn save(&self, layout: &Layout) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        write_atomic(&layout.proposals(), &bytes).map_err(|e| e.to_string())
    }

    /// The proposals waiting for the person.
    #[must_use]
    pub fn waiting(&self) -> Vec<&Proposal> {
        self.folders
            .values()
            .filter(|p| p.status == Status::Proposed)
            .collect()
    }
}

/// One judgment, for the audit log and the Gym's calibration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Judgment {
    pub path: String,
    pub bytes: u64,
    pub setting: String,
    pub probability: f64,
    pub kind: String,
    pub kind_probability: f64,
    pub proposed: bool,
}

/// The folders some class of `rule` covers: a candidate holding one of
/// these, or inside one, is never judged.
fn covered(layout: &Layout, rule: &Rule) -> Vec<PathBuf> {
    let mut covered = vec![
        layout.targets(),
        layout.worktrees(),
        layout.coder_one_target(),
        layout.gate(),
        layout.background(),
        layout.store.clone(),
    ];
    for pattern in &rule.classes.agent_targets {
        covered.extend(rule::glob(pattern, &layout.home));
    }
    for pattern in &rule.classes.checkouts {
        covered.extend(
            rule::glob(pattern, &layout.home)
                .into_iter()
                .map(|checkout| checkout.join("target"))
                .filter(|target| real_dir(target)),
        );
    }
    for judged in &rule.classes.judged {
        covered.push(rule::expand(&judged.path, &layout.home));
    }
    covered
}

/// Why code never asks about `path`, or `None` when it may.
fn excluded(
    layout: &Layout,
    rule: &Rule,
    covered: &[PathBuf],
    path: &Path,
) -> Option<&'static str> {
    if covered
        .iter()
        .any(|c| path.starts_with(c) || c.starts_with(path))
    {
        return Some("a class covers it");
    }
    if layout
        .deny(rule)
        .iter()
        .any(|d| path.starts_with(d) || d.starts_with(path))
    {
        return Some("on the deny list");
    }
    if coder_boundary::privacy::is_protected(path, &layout.home) {
        return Some("a privacy-protected folder");
    }
    if !real_dir(path) || paths::mount_point(path) {
        return Some("a link or another volume");
    }
    if path.join(".git").exists() {
        return Some("a Git checkout");
    }
    None
}

/// The largest folders no class covers, at most [`LARGEST`], each at
/// least [`MIN_BYTES`], skipping any judged within [`AGAIN_SECS`] or
/// declined.
#[must_use]
pub fn survey(env: &Env<'_>, rule: &Rule) -> Vec<Unknown> {
    survey_min(env, rule, MIN_BYTES)
}

/// [`survey`] with another smallest size.
#[must_use]
pub fn survey_min(env: &Env<'_>, rule: &Rule, min_bytes: u64) -> Vec<Unknown> {
    let layout = env.layout;
    let covered = covered(layout, rule);
    let proposals = Proposals::load(layout);
    let snapshot = env.processes.snapshot().ok();
    let mut found = Vec::new();
    for root in SURVEY {
        let root = rule::expand(root, &layout.home);
        if !real_dir(&root) || coder_boundary::privacy::is_protected(&root, &layout.home) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if excluded(layout, rule, &covered, &path).is_some() {
                continue;
            }
            let shown = show(&path, &layout.home);
            if let Some(p) = proposals.folders.get(&shown)
                && (matches!(
                    p.status,
                    Status::Declined | Status::Confirmed | Status::Proposed
                ) || env.now.saturating_sub(p.asked) < AGAIN_SECS)
            {
                continue;
            }
            if let Some(unknown) = look(env, &path, snapshot.as_ref()) {
                found.push(unknown);
            }
        }
    }
    found.retain(|u| u.bytes >= min_bytes);
    found.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path)));
    found.truncate(LARGEST);
    found
}

/// Measure one folder and read its top.
fn look(env: &Env<'_>, path: &Path, snapshot: Option<&crate::inuse::Snapshot>) -> Option<Unknown> {
    let home = &env.layout.home;
    let mut top = Vec::new();
    let mut total = 0u64;
    let mut newest = 0u64;
    let mut markers = Vec::new();
    for entry in std::fs::read_dir(path).ok()?.filter_map(Result::ok) {
        let child = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = std::fs::symlink_metadata(&child) else {
            continue;
        };
        if let Ok(modified) = meta.modified() {
            newest = newest.max(paths::unix(modified));
        }
        if MARKERS.contains(&name.as_str()) {
            markers.push(name.clone());
        }
        if meta.file_type().is_symlink() {
            continue;
        }
        let size = if meta.is_dir() {
            let measured = paths::measure(&child, home).ok()?;
            if measured.foreign {
                return None;
            }
            measured.bytes
        } else {
            std::os::unix::fs::MetadataExt::blocks(&meta) * 512
        };
        total += size;
        top.push((name, size));
    }
    top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top.truncate(8);
    markers.sort();
    Some(Unknown {
        path: path.to_owned(),
        bytes: total,
        age_secs: env.now.saturating_sub(newest),
        top,
        markers,
        users: snapshot.map_or(0, |s| s.inside(path)),
    })
}

/// The questions asked about one folder.
#[must_use]
pub fn questions() -> Vec<(String, Question)> {
    vec![
        ("cache".into(), Question::Noul(QUESTION.into())),
        (
            "kind".into(),
            Question::Choice {
                instructions: "What kind of folder is this?".into(),
                options: ALL_KINDS
                    .iter()
                    .map(|(id, what)| ((*id).to_owned(), (*what).to_owned()))
                    .collect(),
            },
        ),
    ]
}

/// What code makes of Jev's answers: proposed only when the Noul reads at
/// or above the setting and the most likely kind is a disposable one.
#[must_use]
pub fn decide(unknown: &Unknown, home: &Path, p: f64, kind: (&str, f64)) -> Judgment {
    Judgment {
        path: show(&unknown.path, home),
        bytes: unknown.bytes,
        setting: CACHE_DIR.name.into(),
        probability: p,
        kind: kind.0.into(),
        kind_probability: kind.1,
        proposed: CACHE_DIR.yes(p) && KINDS.contains(&kind.0) && unknown.users == 0,
    }
}

/// Judge the largest unknown folders for `rule` and keep what Jev says:
/// proposals for the person, and every judgment for the log. Nothing is
/// deleted or moved.
///
/// # Errors
/// The proposals cannot be saved.
pub fn consider(env: &Env<'_>, rule: &Rule, judge: &dyn Judge) -> Result<Vec<Judgment>, String> {
    consider_min(env, rule, judge, MIN_BYTES)
}

/// [`consider`] over folders of at least `min_bytes`.
///
/// # Errors
/// The proposals cannot be saved.
pub fn consider_min(
    env: &Env<'_>,
    rule: &Rule,
    judge: &dyn Judge,
    min_bytes: u64,
) -> Result<Vec<Judgment>, String> {
    let mut proposals = Proposals::load(env.layout);
    let mut judgments = Vec::new();
    for unknown in survey_min(env, rule, min_bytes) {
        let Ok(answers) = judge.ask(&unknown.state(&env.layout.home), &questions()) else {
            continue;
        };
        let Some(p) = answers.get("cache").and_then(|a| a.noul) else {
            continue;
        };
        let Some(kind) = answers.get("kind").and_then(|a| a.top()) else {
            continue;
        };
        let judgment = decide(&unknown, &env.layout.home, p, kind);
        proposals.folders.insert(
            judgment.path.clone(),
            Proposal {
                path: judgment.path.clone(),
                bytes: judgment.bytes,
                kind: judgment.kind.clone(),
                probability: Some(p),
                kind_probability: Some(kind.1),
                setting: CACHE_DIR.name.into(),
                asked: env.now,
                status: if judgment.proposed {
                    Status::Proposed
                } else {
                    Status::NotCache
                },
                rule: rule.id.clone(),
                plugin: None,
            },
        );
        judgments.push(judgment);
    }
    if !judgments.is_empty() {
        proposals.save(env.layout)?;
    }
    Ok(judgments)
}

/// A plugin's proposed classes (its record's `"classes": [{"path",
/// "kind"}]`): proposals the person confirms like Jev's, never rules.
///
/// # Errors
/// The proposals cannot be saved.
pub fn from_plugin(
    layout: &Layout,
    plugin: &str,
    classes: &[(String, String)],
    now: u64,
) -> Result<usize, String> {
    let mut proposals = Proposals::load(layout);
    let mut added = 0;
    for (path, kind) in classes {
        if proposals.folders.contains_key(path) {
            continue;
        }
        proposals.folders.insert(
            path.clone(),
            Proposal {
                path: path.clone(),
                bytes: 0,
                kind: kind.clone(),
                probability: None,
                kind_probability: None,
                setting: CACHE_DIR.name.into(),
                asked: now,
                status: Status::Proposed,
                rule: "disk".into(),
                plugin: Some(plugin.into()),
            },
        );
        added += 1;
    }
    if added > 0 {
        proposals.save(layout)?;
    }
    Ok(added)
}

/// Confirm the proposal for `path`: the rule it names gets a plain entry
/// for the folder (and a step that moves confirmed caches to the trash),
/// so later runs need no model. A plugin's rule takes it as an edit on
/// this computer, which the host admits since the person confirmed it.
///
/// # Errors
/// No such proposal, it is not a folder a rule may name, or the rule
/// cannot be saved.
pub fn confirm(layout: &Layout, path: &str, now: u64) -> Result<Rule, String> {
    let mut proposals = Proposals::load(layout);
    let proposal = proposals
        .folders
        .get_mut(path)
        .ok_or_else(|| format!("nothing proposed for {path}"))?;
    let expanded = rule::expand(&proposal.path, &layout.home);
    if expanded.join(".git").exists() {
        return Err(format!("{path} is a Git checkout"));
    }
    let mut target = store::load(layout, &proposal.rule).unwrap_or_else(|_| rule::disk());
    if !target
        .classes
        .judged
        .iter()
        .any(|judged| judged.path == proposal.path)
    {
        target.classes.judged.push(Judged {
            path: proposal.path.clone(),
            kind: proposal.kind.clone(),
            confirmed: now,
        });
    }
    let moves = target
        .actions
        .iter()
        .any(|action| action.classes().contains(&Class::Judged));
    if !moves {
        // Before the trash is emptied, after the known classes.
        let at = target
            .actions
            .iter()
            .position(|action| matches!(action, Action::EmptyTrash))
            .unwrap_or(target.actions.len());
        target.actions.insert(
            at,
            Action::DeleteCaches {
                classes: vec![Class::Judged],
            },
        );
    }
    let saved = store::save(layout, &target)?;
    proposal.status = Status::Confirmed;
    proposals.save(layout)?;
    Ok(saved)
}

/// Decline the proposal for `path`: never asked about again.
///
/// # Errors
/// No such proposal, or the file cannot be saved.
pub fn decline(layout: &Layout, path: &str) -> Result<(), String> {
    let mut proposals = Proposals::load(layout);
    let proposal = proposals
        .folders
        .get_mut(path)
        .ok_or_else(|| format!("nothing proposed for {path}"))?;
    proposal.status = Status::Declined;
    proposals.save(layout)
}

/// One line for a proposal.
#[must_use]
pub fn line(proposal: &Proposal) -> String {
    let by = match (&proposal.plugin, proposal.probability) {
        (Some(plugin), _) => format!("proposed by {plugin}"),
        (None, Some(p)) => format!("Jev {p:.2}"),
        (None, None) => String::new(),
    };
    let size = if proposal.bytes > 0 {
        format!(" {}", bytes(proposal.bytes))
    } else {
        String::new()
    };
    format!(
        "{}{size} · {} · {by}",
        proposal.path,
        proposal.kind.replace('_', " ")
    )
}
