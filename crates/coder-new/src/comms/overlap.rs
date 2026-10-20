//! Intents and the overlaps between them.
//!
//! An intent names files, crates and resources. A file under `crates/<name>/`
//! also names that crate. Two intents overlap on a file both name, a file
//! inside a folder the other names, a crate both name, or a resource both
//! name. Paths are compared after [`normal`]: a solver's worktree prefix and
//! a leading `./` are dropped, so `/…/worktree/docs/a.md` and `docs/a.md`
//! are the same file.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// What one member means to change.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intent {
    pub member: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub crates: Vec<String>,
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub at_ms: u64,
    /// When this member first said what it would change; overlaps settled
    /// by order go by this.
    #[serde(default)]
    pub first_ms: u64,
    /// How many times it has said so before (a hand-back restates it).
    #[serde(default)]
    pub round: u32,
}

impl Intent {
    /// An intent with its paths made [`normal`], crates added from them,
    /// and every list sorted without repeats.
    #[must_use]
    pub fn new(
        member: &str,
        files: &[String],
        crates: &[String],
        resources: &[String],
        note: &str,
    ) -> Self {
        let files: BTreeSet<String> = files.iter().filter_map(|file| normal(file)).collect();
        let mut crate_set: BTreeSet<String> = crates
            .iter()
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .collect();
        crate_set.extend(files.iter().filter_map(|file| crate_of(file)));
        let resources: BTreeSet<String> = resources
            .iter()
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .collect();
        let now = super::now_ms();
        Self {
            member: member.to_owned(),
            files: files.into_iter().collect(),
            crates: crate_set.into_iter().collect(),
            resources: resources.into_iter().collect(),
            note: note.trim().to_owned(),
            at_ms: now,
            first_ms: now,
            round: 0,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.crates.is_empty() && self.resources.is_empty()
    }

    /// The intent as one plain sentence.
    #[must_use]
    pub fn sentence(&self) -> String {
        let mut parts = Vec::new();
        if !self.files.is_empty() {
            parts.push(list(&self.files));
        }
        let derived: BTreeSet<String> = self
            .files
            .iter()
            .filter_map(|file| crate_of(file))
            .collect();
        let crates: Vec<String> = self
            .crates
            .iter()
            .filter(|name| !derived.contains(*name))
            .cloned()
            .collect();
        if !crates.is_empty() {
            parts.push(format!("crate {}", list(&crates)));
        }
        if !self.resources.is_empty() {
            parts.push(list(&self.resources));
        }
        let verb = if self.round == 0 {
            "will change"
        } else {
            "now changes"
        };
        let mut text = format!("{} {verb} {}.", self.member, parts.join("; "));
        if !self.note.is_empty() {
            text.push(' ');
            text.push_str(&self.note);
        }
        text
    }
}

/// What two members' intents share.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overlap {
    /// The member whose intent was just posted.
    pub a: String,
    pub b: String,
    pub files: Vec<String>,
    pub crates: Vec<String>,
    pub resources: Vec<String>,
}

impl Overlap {
    /// Everything shared, files first.
    #[must_use]
    pub fn shared(&self) -> Vec<String> {
        let mut shared = self.files.clone();
        let derived: BTreeSet<String> = self
            .files
            .iter()
            .filter_map(|file| crate_of(file))
            .collect();
        shared.extend(
            self.crates
                .iter()
                .filter(|name| !derived.contains(*name))
                .map(|name| format!("crate {name}")),
        );
        shared.extend(self.resources.iter().cloned());
        shared
    }

    /// The pair, the same whichever member posted.
    #[must_use]
    pub fn pair_key(&self) -> String {
        let mut pair = [self.a.as_str(), self.b.as_str()];
        pair.sort_unstable();
        pair.join("\u{1f}")
    }

    /// The notice both members read.
    #[must_use]
    pub fn sentence(&self) -> String {
        format!(
            "{} and {} overlap on {}. Agree who goes first, split the change, or merge into one pull request.",
            self.a,
            self.b,
            list(&self.shared())
        )
    }
}

/// What `a` and `b` share, or `None`.
#[must_use]
pub fn between(a: &Intent, b: &Intent) -> Option<Overlap> {
    let mut files = BTreeSet::new();
    for left in &a.files {
        for right in &b.files {
            if left == right || inside(left, right) {
                files.insert(left.clone());
            } else if inside(right, left) {
                files.insert(right.clone());
            }
        }
    }
    let crates: Vec<String> = a
        .crates
        .iter()
        .filter(|name| b.crates.contains(name))
        .cloned()
        .collect();
    let resources: Vec<String> = a
        .resources
        .iter()
        .filter(|name| b.resources.contains(name))
        .cloned()
        .collect();
    if files.is_empty() && crates.is_empty() && resources.is_empty() {
        return None;
    }
    Some(Overlap {
        a: a.member.clone(),
        b: b.member.clone(),
        files: files.into_iter().collect(),
        crates,
        resources,
    })
}

/// Whether `path` is inside the folder `folder`.
fn inside(path: &str, folder: &str) -> bool {
    path.len() > folder.len() && path.starts_with(folder) && path.as_bytes()[folder.len()] == b'/'
}

/// A path as intents compare it: the part after a solver's `worktree/`,
/// without a leading `./` or a trailing `/`. `None` for an empty path.
#[must_use]
pub fn normal(path: &str) -> Option<String> {
    let mut path = path.trim().trim_matches('`');
    if let Some(at) = path.rfind("/worktree/") {
        path = &path[at + "/worktree/".len()..];
    }
    while let Some(rest) = path.strip_prefix("./") {
        path = rest;
    }
    let path = path.trim_end_matches('/');
    (!path.is_empty() && path != ".").then(|| path.to_owned())
}

/// The crate a path is in: `crates/<name>/...` names `<name>`.
#[must_use]
pub fn crate_of(path: &str) -> Option<String> {
    let rest = path.strip_prefix("crates/")?;
    let (name, _) = rest.split_once('/')?;
    (!name.is_empty()).then(|| name.to_owned())
}

/// `a`, `a and b`, `a, b and c`.
fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}
