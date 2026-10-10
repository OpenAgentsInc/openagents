//! What the landing queue knows about an entry before it lands (#11248).
//!
//! The queue used to take entries strictly one at a time, so a docs fix
//! waited behind Rust changes whose checks take half an hour each. A
//! [`Plan`] says which lane an entry takes and what it can affect:
//!
//! - **The fast lane** ([`Lane::Fast`]): every changed path is a document
//!   or an image under `docs/`, `nips/` or the top of the repository, and
//!   no file the build reads names it ([`refers`]). It skips the build and
//!   gets the diff checks only.
//! - **The code lane** ([`Lane::Code`]): everything else. Unsure means
//!   code. Its [`Plan::packages`] are the workspace packages whose
//!   directories it touches, the packages whose sources name a changed file
//!   outside any package (an `include_str!` of a script, a test's fixture),
//!   and pseudo-packages (`dir:crates/psionic`) for build directories
//!   outside the workspace. Its [`Plan::closure`] adds what those packages
//!   depend on and what depends on them.
//!
//! Two code entries [`overlap`] when one's packages are in the other's
//! closure (the relation [`super::landing::affects`] uses to decide whether
//! newly landed commits re-run checks), when they change the same file, or
//! when either changes a workspace-wide build file. Overlapping entries
//! land one at a time in submission order; the others run side by side.
//!
//! The registry of generated files ([`Generator`],
//! `.openagents/generated.json`) lets the queue rewrite a committed file
//! that is generated from sources (the CLI tree, the first-party OpenAPI
//! document) when the change touches those sources, instead of bouncing
//! the entry or turning `main` red.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Which lane an entry lands through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    /// No compiled code: the diff checks only, in seconds.
    Fast,
    /// The touched packages' checks, side by side with entries it does
    /// not overlap.
    Code,
}

impl Lane {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Lane::Fast => "fast",
            Lane::Code => "code",
        }
    }
}

/// What an entry changes and what it can affect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub lane: Lane,
    /// Every path the change adds, edits or deletes.
    pub files: Vec<String>,
    /// The packages it touches or reaches through a file their sources
    /// name.
    pub packages: Vec<String>,
    /// The packages it reaches only through a file their sources name;
    /// the checks test these too.
    pub also: Vec<String>,
    /// The packages, what they depend on, and what depends on them.
    pub closure: Vec<String>,
    /// It changes a file every package builds with (or the package graph
    /// could not be read): it overlaps every code entry.
    pub wide: bool,
    /// Why, in a few words, for `openagents land status`.
    pub why: String,
}

impl Plan {
    /// The plan of an entry that could not be read: code, overlapping
    /// everything, so it waits its turn as every entry once did.
    #[must_use]
    pub fn unknown(why: &str) -> Plan {
        Plan {
            lane: Lane::Code,
            files: Vec::new(),
            packages: Vec::new(),
            also: Vec::new(),
            closure: Vec::new(),
            wide: true,
            why: format!("not planned ({why}); treated as overlapping every code entry"),
        }
    }
}

/// Extensions of files no build reads unless a source names them.
const DOCUMENT_EXTENSIONS: &[&str] = &[
    "md", "markdown", "txt", "rst", "adoc", "png", "jpg", "jpeg", "gif", "webp", "svg", "ico",
    "pdf", "mp4", "mov", "webm",
];

/// Folders whose documents can take the fast lane. Everything else in the
/// repository (crates, apps, fixtures, knowledge, plugins, assets...) is
/// read by some build or test, so it is code.
const DOCUMENT_ROOTS: &[&str] = &["docs/", "nips/"];

/// Files that make every package build differently.
fn workspace_wide(path: &str) -> bool {
    super::landing::workspace_wide(path)
        || matches!(
            path,
            "Cargo.lock" | "rustfmt.toml" | "clippy.toml" | "deny.toml" | ".cargo/config.toml"
        )
}

/// Whether `path` could take the fast lane by where it is and what it is:
/// a document or image under [`DOCUMENT_ROOTS`] or at the top of the
/// repository. A source may still name it ([`refers`]).
#[must_use]
pub fn document(path: &str) -> bool {
    let Some((_, extension)) = path.rsplit_once('.') else {
        return false;
    };
    let extension = extension.to_ascii_lowercase();
    if !DOCUMENT_EXTENSIONS.contains(&extension.as_str()) {
        return false;
    }
    let at_top = !path.contains('/');
    at_top || DOCUMENT_ROOTS.iter().any(|root| path.starts_with(root))
}

/// A quoted string in a file the build reads that looks like a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Literal {
    /// The file it is in, from the top of the repository.
    pub file: String,
    /// The string, without its quotes.
    pub text: String,
}

/// `a/b/../c` → `a/c`; `None` when it climbs above the top.
fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// Whether the string `text` in `file` names `path` (from the top of the
/// repository): as a path relative to the file (`../../docs/x.md`), from
/// the top (`docs/x.md`), as a path tail of two or more parts that an
/// `include_str!` reaches with `../` (`coder-one/prompts/role.md`), or as a
/// folder or stem of two or more parts the path is in
/// (`docs/coder/measurements/`, `.../2026-10-01-essays-summary-claims`). A
/// template (`docs/{name}.md`) names what its fixed start does. A
/// one-part folder (`../../../docs/`, the start of a `concat!` whose tails
/// name the files) names nothing by itself.
#[must_use]
pub fn refers(file: &str, text: &str, path: &str) -> bool {
    let text = text.split('{').next().unwrap_or("");
    if text.is_empty() {
        return false;
    }
    // Cheap first: whatever form it takes, a string that names `path`
    // holds its last non-empty part.
    let last = text.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    if last.is_empty() || last == "." || last == ".." || !path.contains(last) {
        return false;
    }
    if text.starts_with("./") || text.starts_with("../") {
        let dir = file.rsplit_once('/').map_or("", |(dir, _)| dir);
        if let Some(resolved) = normalize(&format!("{dir}/{text}"))
            && !resolved.is_empty()
            && (resolved == path
                || (resolved.contains('/') && path.starts_with(&format!("{resolved}/"))))
        {
            return true;
        }
    }
    let mut tail = text;
    loop {
        let before = tail;
        tail = tail.trim_start_matches('/');
        tail = tail.strip_prefix("./").unwrap_or(tail);
        tail = tail.strip_prefix("../").unwrap_or(tail);
        if tail == before {
            break;
        }
    }
    if tail.is_empty() {
        return false;
    }
    let parts = tail.trim_end_matches('/').split('/').count();
    // A one-part name (`AGENTS.md`) is the top's file only in Rust code
    // that joins it to the checkout; a manifest's or a script's bare
    // `README.md` is its own folder's. A relative `../README.md` was
    // resolved above.
    let relative = text.starts_with("./") || text.starts_with("../");
    if tail == path && (parts >= 2 || (!relative && file.ends_with(".rs"))) {
        return true;
    }
    if parts < 2 {
        return false;
    }
    if path.len() > tail.len()
        && path.ends_with(tail)
        && path.as_bytes()[path.len() - tail.len() - 1] == b'/'
    {
        return true;
    }
    if let Some(rest) = path.strip_prefix(tail) {
        return tail.ends_with('/') || rest.starts_with('/') || rest.starts_with('.');
    }
    false
}

/// Each workspace package's workspace dependencies.
pub type Graph = Vec<(String, Vec<String>)>;

/// `packages`, every workspace package they depend on, and every one that
/// depends on them.
#[must_use]
pub fn closure(packages: &[String], graph: &Graph) -> Vec<String> {
    let mut out: BTreeSet<String> = packages.iter().cloned().collect();
    for (name, _) in graph {
        if packages.iter().any(|package| {
            super::landing::reaches(graph, name, package)
                || super::landing::reaches(graph, package, name)
        }) {
            out.insert(name.clone());
        }
    }
    out.into_iter().collect()
}

/// Whether two entries must land one after the other: both code, and one
/// can change what the other's checks test, or they change the same file,
/// or either changes what every package builds with.
#[must_use]
pub fn overlap(a: &Plan, b: &Plan) -> bool {
    if a.lane == Lane::Fast || b.lane == Lane::Fast {
        return false;
    }
    if a.wide || b.wide {
        return true;
    }
    if a.files.iter().any(|file| b.files.contains(file)) {
        return true;
    }
    a.packages.iter().any(|p| b.closure.contains(p))
        || b.packages.iter().any(|p| a.closure.contains(p))
}

/// What the repository says about the changed paths, for [`classify`].
pub struct Facts<'a> {
    /// The workspace package a path is in, or the build folder outside
    /// the workspace (`dir:crates/psionic`); `None` for neither.
    pub package_of: &'a dyn Fn(&str) -> Option<String>,
    /// Strings that look like paths in the files the build reads.
    pub literals: &'a [Literal],
    /// The package graph; `None` when it could not be read.
    pub graph: Option<&'a Graph>,
}

/// Keeps the first reason only.
fn first(slot: &mut Option<String>, why: impl FnOnce() -> String) {
    if slot.is_none() {
        *slot = Some(why());
    }
}

/// The plan for a change to `files`.
#[must_use]
pub fn classify(files: &[String], facts: &Facts<'_>) -> Plan {
    let mut packages: Vec<String> = Vec::new();
    let mut also: Vec<String> = Vec::new();
    let mut code_because: Option<String> = None;
    let mut wide = false;
    for path in files {
        if workspace_wide(path) {
            wide = true;
            first(&mut code_because, || {
                format!("`{path}` changes every build")
            });
        }
        let home = (facts.package_of)(path);
        if let Some(package) = &home {
            if !packages.contains(package) {
                packages.push(package.clone());
            }
            first(&mut code_because, || format!("`{path}` is in `{package}`"));
            // Its package's checks already cover what reads it.
            continue;
        }
        let mut named = false;
        for literal in facts.literals {
            if literal.file == *path || !refers(&literal.file, &literal.text, path) {
                continue;
            }
            named = true;
            if let Some(package) = (facts.package_of)(&literal.file)
                && !package.starts_with("dir:")
            {
                if !packages.contains(&package) {
                    packages.push(package.clone());
                }
                if !also.contains(&package) {
                    also.push(package);
                }
            }
        }
        if named {
            first(&mut code_because, || {
                format!("`{path}` is read by the build")
            });
        } else if !document(path) {
            first(&mut code_because, || format!("`{path}` is not a document"));
        }
    }
    let Some(why) = code_because else {
        return Plan {
            lane: Lane::Fast,
            files: files.to_vec(),
            packages: Vec::new(),
            also: Vec::new(),
            closure: Vec::new(),
            wide: false,
            why: format!("{} document(s), no build reads them", files.len()),
        };
    };
    let members: Vec<String> = packages
        .iter()
        .filter(|p| !p.starts_with("dir:"))
        .cloned()
        .collect();
    let closure = match facts.graph {
        Some(graph) => {
            let mut all = closure(&members, graph);
            for p in packages.iter().filter(|p| p.starts_with("dir:")) {
                all.push(p.clone());
            }
            all
        }
        None => {
            if !members.is_empty() {
                wide = true;
            }
            packages.clone()
        }
    };
    Plan {
        lane: Lane::Code,
        files: files.to_vec(),
        packages,
        also,
        closure,
        wide,
        why,
    }
}

/// Folder markers of builds outside the Cargo workspace.
const BUILD_MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "Package.swift",
    "build.gradle",
    "build.gradle.kts",
    "go.mod",
    "pyproject.toml",
];

/// The package `path` is in, read from the checkout at `worktree`: a
/// workspace member's name, or `dir:<folder>` for the nearest folder with
/// a build manifest that is not one (its own workspace, an app).
#[must_use]
pub fn package_of(worktree: &Path, path: &str) -> Option<String> {
    if let Some(package) = super::landing::packages(worktree, &[path.to_owned()])
        .into_iter()
        .next()
    {
        return Some(package);
    }
    let mut dir = Path::new(path).parent();
    while let Some(at) = dir {
        if at.as_os_str().is_empty() {
            break;
        }
        if BUILD_MANIFESTS
            .iter()
            .any(|manifest| worktree.join(at).join(manifest).is_file())
        {
            return Some(format!("dir:{}", at.display()));
        }
        dir = at.parent();
    }
    None
}

/// The files whose quoted paths [`literals`] reads: what a build, a test
/// or a deploy can read. Documents under [`DOCUMENT_ROOTS`] are left out:
/// a document naming another does not make it code.
const SOURCE_SPECS: &[&str] = &[
    "*.rs",
    "*.toml",
    "*.json",
    "*.ts",
    "*.tsx",
    "*.js",
    "*.mjs",
    "*.cjs",
    "*.swift",
    "*.kt",
    "*.kts",
    "*.sh",
    "*.py",
    "*.yml",
    "*.yaml",
    "Dockerfile*",
    "Makefile",
    ":!docs/**",
    ":!nips/**",
];

/// Every quoted string with a `/`, or ending in a document's extension,
/// in the files the build reads at `worktree`'s `HEAD`.
///
/// # Errors
/// Git could not search the checkout.
pub fn literals(worktree: &Path) -> Result<Vec<Literal>, String> {
    let extensions = DOCUMENT_EXTENSIONS.join("|");
    let pattern = format!(r#""[^"[:space:]]*/[^"[:space:]]*"|"[^"[:space:]/]+\.({extensions})""#);
    let mut args = vec![
        "grep",
        "-I",
        "-o",
        "-E",
        "--full-name",
        "-e",
        pattern.as_str(),
        "--",
    ];
    args.extend_from_slice(SOURCE_SPECS);
    let output = std::process::Command::new("git")
        .args(&args)
        .current_dir(worktree)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("cannot run git grep: {e}"))?;
    // 1 is "nothing found".
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let mut found = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((file, quoted)) = line.split_once(':') else {
            continue;
        };
        let text = quoted.trim_matches('"');
        if text.contains("://") || text.len() < 3 {
            continue;
        }
        found.push(Literal {
            file: file.to_owned(),
            text: text.to_owned(),
        });
    }
    Ok(found)
}

/// The plan for the change from `base` to `worktree`'s `HEAD`.
#[must_use]
pub fn plan(worktree: &Path, base: &str) -> Plan {
    let changed = std::process::Command::new("git")
        .args(["diff", "--name-only", "--no-renames", base, "HEAD"])
        .current_dir(worktree)
        .stdin(std::process::Stdio::null())
        .output();
    let files: Vec<String> = match changed {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect(),
        Ok(out) => return Plan::unknown(String::from_utf8_lossy(&out.stderr).trim()),
        Err(e) => return Plan::unknown(&e.to_string()),
    };
    let literals = match literals(worktree) {
        Ok(literals) => literals,
        Err(why) => return Plan::unknown(&format!("git grep failed: {why}")),
    };
    let package_of = |path: &str| package_of(worktree, path);
    // The graph only matters to code; reading it runs cargo.
    let needs_graph = files.iter().any(|f| !document(f));
    let graph = if needs_graph {
        super::landing::workspace_graph(worktree)
    } else {
        Some(Vec::new())
    };
    let mut plan = classify(
        &files,
        &Facts {
            package_of: &package_of,
            literals: &literals,
            graph: graph.as_ref(),
        },
    );
    if plan.lane == Lane::Code && graph.is_none() && plan.packages.is_empty() {
        // Named-by-literal documents with an unreadable graph stay code.
        plan.wide = true;
    }
    plan
}

// ---------------------------------------------------------------------------
// Generated files.

/// The registry of generated files, at the top of the repository.
pub const GENERATED_FILE: &str = ".openagents/generated.json";

#[derive(Clone, Debug, Default, Deserialize)]
struct Registry {
    #[serde(default)]
    generated: Vec<Generator>,
}

/// One committed file (or a few) generated from sources, as
/// [`GENERATED_FILE`] declares it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Generator {
    pub name: String,
    /// The files the command writes.
    pub files: Vec<String>,
    /// Path prefixes of the sources: a change under one is due a rewrite.
    pub sources: Vec<String>,
    /// When present, a changed source is due only when it contains one of
    /// these words (the CLI tree comes from `USAGE` strings and effect
    /// declarations, not every line of the CLI).
    #[serde(default)]
    pub markers: Vec<String>,
    /// The shell command, run at the top of the repository, that rewrites
    /// the files.
    pub regenerate: String,
}

/// The generators `worktree` declares; none when it declares none.
#[must_use]
pub fn generators(worktree: &Path) -> Vec<Generator> {
    std::fs::read(worktree.join(GENERATED_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Registry>(&bytes).ok())
        .map(|registry| registry.generated)
        .unwrap_or_default()
}

/// The generators a change to `files` is due: a changed file under one of
/// its sources that holds one of its markers (`read` gives a file's text).
#[must_use]
pub fn due<'a>(
    generators: &'a [Generator],
    files: &[String],
    read: &dyn Fn(&str) -> Option<String>,
) -> Vec<&'a Generator> {
    generators
        .iter()
        .filter(|generator| {
            files.iter().any(|file| {
                !generator.files.contains(file)
                    && generator.sources.iter().any(|s| file.starts_with(s))
                    && (generator.markers.is_empty()
                        || read(file).is_some_and(|text| {
                            generator.markers.iter().any(|m| text.contains(m.as_str()))
                        }))
            })
        })
        .collect()
}

/// Whether every one of `paths` is a generated file, so a rebase conflict
/// in them is settled by taking the target's copy and generating again.
#[must_use]
pub fn all_generated(generators: &[Generator], paths: &[String]) -> bool {
    !paths.is_empty()
        && paths
            .iter()
            .all(|path| generators.iter().any(|g| g.files.contains(path)))
}

#[cfg(test)]
#[path = "land_plan_tests.rs"]
mod tests;
