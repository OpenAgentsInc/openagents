//! Capabilities: what this machine can hand work to, and whether it will.
//!
//! A capability is something a Coder instance can reach — another agent's
//! CLI on the same computer, a cloud lane, a subprocess with a protocol.
//! [NIP-CAP](../../../nips/openagents/NIP-CAP.md) defines the manifest that
//! says how to drive one: the transport, what to run to detect it, the
//! bounds it keeps, and **the bounds it will silently ignore**. This module
//! reads a manifest from a local file and probes the machine with it.
//!
//! # Three states, not two
//!
//! A probe answers with a [`Presence`], and the third state is the reason
//! this is a module rather than a `which` call:
//!
//! - [`Presence::Present`] — `detect` resolved and the version parsed.
//! - [`Presence::Absent`] — nothing to run. **Not an error.** An operator
//!   without Devin loses nothing, because an absent capability is not an
//!   option rather than a failure.
//! - [`Presence::Unavailable`] — installed, detected, reporting its
//!   version, and refusing this context.
//!
//! The third one was recorded before it was implemented. Six of six
//! delegations in `crates/coderbench`'s golden were declined with
//! `Refusing to run in an untrusted workspace` from a git worktree under
//! `/private/tmp`, with the executor present the whole time. A host that
//! reads that as present offers a route that fails every time it is taken,
//! and a host that reads it as absent cannot say why a capability the
//! operator installed is missing.
//!
//! # `PATH` is a hint, not the answer
//!
//! The probe resolves an absolute path and runs that. `devin` sat on the
//! operator's interactive `PATH` and not on the one a spawned subshell
//! inherited, and six delegations failed with `command not found` before
//! the full path was resolved. So [`search_dirs`] reads `PATH` for
//! candidate directories, then keeps looking in the directories a login
//! shell usually adds, and every argv the probe runs starts with the
//! resolved path rather than a bare name.
//!
//! A manifest is untrusted input. Its `detect` is a fixed argv this module
//! spawns directly, never a string it hands to a shell.

use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use atif::{Call, Outcome};
use serde::{Deserialize, Serialize};
use serde_json::{Map, json};

/// The event kind a manifest publishes as, per NIP-CAP.
pub const MANIFEST_KIND: u16 = 30180;

/// The manifest body version this reads. A host refuses a `v` it does not
/// know rather than reading the fields it recognizes.
pub const MANIFEST_VERSION: u32 = 1;

/// The name a probe records itself under in a trace.
pub const PROBE_CALL: &str = "capability_probe";

/// The variable that moves the manifest directory.
pub const DIR_ENV: &str = "CODER_CAPABILITY_DIR";

/// The variable that adds directories to the executable search, ahead of
/// everything else.
pub const PATH_ENV: &str = "CODER_CAPABILITY_PATH";

/// Directories a login shell usually adds and a spawned subshell often
/// does not. Searched after `PATH`, which is where the operator's own
/// answer lives when it survives.
const FALLBACK_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
];

/// The same, under the user's home directory.
const HOME_DIRS: &[&str] = &[".local/bin", "bin", ".cargo/bin", ".bun/bin"];

/// One capability manifest: how to drive one executor.
///
/// `slug` and `name` are the `d` and `name` tags the published
/// `kind:30180` carries; everything else is the body. The file on disk
/// holds them together so one document is one manifest.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Manifest {
    /// The body schema version.
    pub v: u32,
    /// The capability slug — the `d` tag.
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: String,
    /// How the host speaks to it: `acp`, `subprocess`, `http`.
    pub transport: String,
    pub detect: Detect,
    /// Bounds the executor will hold to if given.
    #[serde(default)]
    pub enforces: Vec<String>,
    /// Bounds it will silently ignore. Stated positively because an
    /// omission is ambiguous and a refusal must not rest on one.
    #[serde(default)]
    pub cannot_enforce: Vec<String>,
    /// Whether the executor can read the caller's working directory.
    #[serde(default)]
    pub sees_repository: bool,
    /// The most simultaneous instances the manifest claims are safe.
    #[serde(default)]
    pub concurrent_max: Option<u32>,
    /// Who pays: `operator_account`, `metered`, `local`.
    #[serde(default)]
    pub cost: String,
    /// Which checkout shapes it accepts.
    #[serde(default)]
    pub isolation: Vec<String>,
    /// The argv that hands this executor a task, with the prompt appended
    /// as the final argument. Empty means a host can detect the executor
    /// and cannot drive it, which is a manifest that describes something
    /// without saying how to use it.
    #[serde(default)]
    pub invoke: Vec<String>,
    /// What to run in a candidate working directory to find out whether
    /// the executor will accept it.
    #[serde(default)]
    pub workspace_probe: Option<WorkspaceProbe>,
    /// What the executor declines, beyond being absent.
    #[serde(default)]
    pub refuses: Vec<Refusal>,
}

/// What a host runs to decide the executor is present and to read its
/// version. Each is a fixed argv, never a shell string.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Detect {
    pub binary: String,
    pub version: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe: Option<Vec<String>>,
}

/// The argv that asks the executor whether it will work in a directory.
///
/// It runs in the candidate workspace and is read for a declared
/// [`Refusal`]. Whether it succeeds is not the question: an argv that stops
/// short of doing any work is the right one, because a probe that started a
/// session would cost the operator something every time a host looked.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkspaceProbe {
    pub argv: Vec<String>,
    #[serde(default)]
    pub note: String,
}

/// One refusal the executor is known to answer with, and the text that
/// identifies it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Refusal {
    pub name: String,
    /// The text the executor prints when it refuses.
    #[serde(rename = "match")]
    pub matches: String,
    #[serde(default)]
    pub explanation: String,
}

/// What a probe found, in three states.
///
/// The variants are deliberately not `Option<Something>`: absence and
/// refusal are different answers with different consequences, and a type
/// that collapses them makes the mistake this module exists to prevent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presence {
    /// `detect` resolved and the version parsed.
    Present {
        /// The version as it parsed — `3000.10.31`.
        version: String,
        /// The first line the version command printed, whole.
        report: String,
        /// The absolute path the host will run.
        path: PathBuf,
    },
    /// Nothing to run. Not an error.
    Absent {
        /// Why there is nothing to run, in a sentence.
        reason: String,
        /// Where the host looked, so an operator who expected it can see
        /// which directory to put it in.
        looked_in: Vec<PathBuf>,
    },
    /// Installed, detected, and refusing this context.
    Unavailable {
        version: String,
        report: String,
        path: PathBuf,
        /// The declared refusal's name.
        refusal: String,
        /// What the executor actually printed.
        detail: String,
    },
}

impl Presence {
    /// Whether a host may offer this capability as a route.
    ///
    /// Both of the other two states answer `false`, and for the same
    /// practical reason: a route that cannot be taken is not a route.
    #[must_use]
    pub fn available(&self) -> bool {
        matches!(self, Presence::Present { .. })
    }

    /// Whether the executor is on the machine at all, refusing or not.
    #[must_use]
    pub fn installed(&self) -> bool {
        !matches!(self, Presence::Absent { .. })
    }

    /// The state's name, as a trace records it.
    #[must_use]
    pub fn state(&self) -> &'static str {
        match self {
            Presence::Present { .. } => "present",
            Presence::Absent { .. } => "absent",
            Presence::Unavailable { .. } => "present_unavailable",
        }
    }
}

/// One capability, probed on one machine in one workspace.
#[derive(Clone, Debug)]
pub struct Found {
    /// The manifest the probe read.
    pub manifest: Manifest,
    /// What it found.
    pub presence: Presence,
    /// The directory the probe asked about.
    pub workspace: PathBuf,
    /// Wall time the probe took.
    pub milliseconds: u64,
}

impl Found {
    /// The capability slug.
    #[must_use]
    pub fn capability(&self) -> &str {
        &self.manifest.slug
    }

    /// Whether a host may offer this capability as a route.
    #[must_use]
    pub fn available(&self) -> bool {
        self.presence.available()
    }

    /// The sentence a surface shows beside the probe.
    #[must_use]
    pub fn message(&self) -> String {
        let slug = &self.manifest.slug;
        match &self.presence {
            Presence::Present { report, .. } => format!("{slug} is present: {report}."),
            Presence::Absent { reason, .. } => format!("{slug} is absent: {reason}."),
            Presence::Unavailable { refusal, .. } => {
                format!("{slug} is present and unavailable here: {refusal}.")
            }
        }
    }

    /// What the probe answered, in one line.
    #[must_use]
    pub fn output(&self) -> String {
        match &self.presence {
            Presence::Present { report, path, .. } => format!("{report} at {}", path.display()),
            Presence::Absent { reason, .. } => reason.clone(),
            Presence::Unavailable {
                report,
                path,
                refusal,
                ..
            } => format!(
                "{report} at {}, refusing this workspace: {refusal}",
                path.display()
            ),
        }
    }

    /// The probe as a trace records it.
    ///
    /// The outcome is `Completed` in all three states, including absence.
    /// A capability that is not installed is an answer the host asked for
    /// and got; recording it as a failed call would put a fault in every
    /// trace on every machine that does not have Devin.
    #[must_use]
    pub fn call(&self) -> Call {
        let manifest = &self.manifest;
        let mut extra = Map::new();
        extra.insert("capability".to_string(), json!(manifest.slug));
        extra.insert("state".to_string(), json!(self.presence.state()));
        extra.insert("present".to_string(), json!(self.presence.installed()));
        extra.insert("available".to_string(), json!(self.presence.available()));
        extra.insert("transport".to_string(), json!(manifest.transport));
        extra.insert("enforces".to_string(), json!(manifest.enforces));
        extra.insert("cannot_enforce".to_string(), json!(manifest.cannot_enforce));
        extra.insert(
            "sees_repository".to_string(),
            json!(manifest.sees_repository),
        );
        extra.insert(
            "refuses".to_string(),
            json!(
                manifest
                    .refuses
                    .iter()
                    .map(|refusal| refusal.name.clone())
                    .collect::<Vec<_>>()
            ),
        );
        extra.insert(
            "workspace".to_string(),
            json!(self.workspace.display().to_string()),
        );
        match &self.presence {
            Presence::Present { version, path, .. } => {
                extra.insert("version".to_string(), json!(version));
                extra.insert(
                    "resolved_path".to_string(),
                    json!(path.display().to_string()),
                );
            }
            Presence::Absent { reason, looked_in } => {
                extra.insert("reason".to_string(), json!(reason));
                extra.insert(
                    "looked_in".to_string(),
                    json!(
                        looked_in
                            .iter()
                            .map(|dir| dir.display().to_string())
                            .collect::<Vec<_>>()
                    ),
                );
            }
            Presence::Unavailable {
                version,
                path,
                refusal,
                detail,
                ..
            } => {
                extra.insert("version".to_string(), json!(version));
                extra.insert(
                    "resolved_path".to_string(),
                    json!(path.display().to_string()),
                );
                let explanation = manifest
                    .refuses
                    .iter()
                    .find(|declared| &declared.name == refusal)
                    .map(|declared| declared.explanation.clone())
                    .unwrap_or_default();
                extra.insert(
                    "refused".to_string(),
                    json!({
                        "name": refusal,
                        "detail": detail,
                        "explanation": explanation,
                        "workspace": self.workspace.display().to_string(),
                    }),
                );
            }
        }
        Call {
            id: String::new(),
            name: PROBE_CALL.to_string(),
            arguments: json!({ "detect": manifest.detect }),
            output: self.output(),
            outcome: Outcome::Completed,
            milliseconds: self.milliseconds,
            purpose: Some(format!(
                "Resolve {} before offering it as a route.",
                manifest.slug
            )),
            extra,
        }
    }
}

impl Manifest {
    /// Reads a manifest from a local file.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the file is not a manifest this
    /// version runs: unreadable, unparseable, a `v` it does not know, a
    /// slug outside the NIP-CAP grammar, or a `detect` with no argv. A
    /// manifest a host half-understands drives an executor by a rule
    /// nobody stated.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let manifest: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        manifest
            .validate()
            .map_err(|reason| format!("{}: {reason}", path.display()))?;
        Ok(manifest)
    }

    /// Whether this manifest is one this version runs.
    ///
    /// # Errors
    ///
    /// Returns the first reason it is not.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != MANIFEST_VERSION {
            return Err(format!(
                "body version is {}, this version reads {MANIFEST_VERSION}",
                self.v
            ));
        }
        if !is_slug(&self.slug) {
            return Err(format!("slug {:?} is not a capability slug", self.slug));
        }
        if self.detect.binary.is_empty() {
            return Err("detect names no binary".to_string());
        }
        if self.detect.version.is_empty() {
            return Err("detect carries no version argv".to_string());
        }
        if let Some(probe) = &self.workspace_probe
            && probe.argv.is_empty()
        {
            return Err("the workspace probe carries no argv".to_string());
        }
        Ok(())
    }

    /// The bounds this executor would silently ignore, out of the ones a
    /// delegation needs. An empty answer admits the delegation.
    ///
    /// The intersection with `cannot_enforce` is the whole test. An
    /// executor that ignores a bound is more dangerous than one that
    /// refuses it, so a host refuses the pairing rather than issuing it and
    /// hoping.
    #[must_use]
    pub fn ignored_bounds(&self, required: &[String]) -> Vec<String> {
        required
            .iter()
            .filter(|bound| self.cannot_enforce.contains(bound))
            .cloned()
            .collect()
    }

    /// Probes this machine for the executor, asking about one workspace.
    ///
    /// Never returns an error. Every outcome a probe can have is one of the
    /// three states, and a caller that has to handle a fourth would start
    /// treating absence as one.
    #[must_use]
    pub fn probe(&self, workspace: &Path) -> Found {
        let started = Instant::now();
        let presence = self.presence(workspace);
        Found {
            manifest: self.clone(),
            presence,
            workspace: workspace.to_path_buf(),
            milliseconds: started.elapsed().as_millis() as u64,
        }
    }

    fn presence(&self, workspace: &Path) -> Presence {
        let dirs = search_dirs();
        let Some(path) = resolve(&self.detect.binary, &dirs) else {
            return Presence::Absent {
                reason: format!(
                    "no {} in {} directories on this machine",
                    self.detect.binary,
                    dirs.len()
                ),
                looked_in: dirs,
            };
        };
        let version = match run(&path, &self.detect.version, workspace) {
            Ok(output) => output,
            Err(error) => {
                return Presence::Absent {
                    reason: format!("{} will not report a version: {error}", path.display()),
                    looked_in: dirs,
                };
            }
        };
        let report = first_line(&version).to_string();
        let Some(version) = version_in(&report) else {
            return Presence::Absent {
                reason: format!(
                    "{} printed {report:?}, which carries no version",
                    path.display()
                ),
                looked_in: dirs,
            };
        };
        let Some(probe) = &self.workspace_probe else {
            return Presence::Present {
                version,
                report,
                path,
            };
        };
        // A workspace probe that will not run proves nothing, so it cannot
        // manufacture a refusal. The capability stays present, which is
        // what the version command already established.
        let Ok(answer) = run(&path, &probe.argv, workspace) else {
            return Presence::Present {
                version,
                report,
                path,
            };
        };
        match self
            .refuses
            .iter()
            .find(|refusal| answer.contains(&refusal.matches))
        {
            Some(refusal) => Presence::Unavailable {
                version,
                report,
                path,
                refusal: refusal.name.clone(),
                detail: first_line(&answer).to_string(),
            },
            None => Presence::Present {
                version,
                report,
                path,
            },
        }
    }
}

/// The capability manifests a host has read.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    manifests: Vec<Manifest>,
    refused: Vec<(String, String)>,
}

impl Registry {
    /// Reads every manifest in one directory, in slug order.
    ///
    /// A file that is not a manifest this version runs is refused by name
    /// rather than skipped silently, because an operator who wrote one and
    /// sees nothing has no way to tell which of the two happened.
    ///
    /// # Errors
    ///
    /// Returns the underlying error when the directory cannot be read.
    pub fn read(dir: &Path) -> Result<Self, String> {
        let mut manifests = Vec::new();
        let mut refused = Vec::new();
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();
        for path in paths {
            match Manifest::load(&path) {
                Ok(manifest) => manifests.push(manifest),
                Err(reason) => refused.push((path.display().to_string(), reason)),
            }
        }
        manifests.sort_by(|a, b| a.slug.cmp(&b.slug));
        Ok(Registry { manifests, refused })
    }

    /// Reads each directory in `dirs` in turn and merges what they hold.
    /// The first definition of a slug wins, so a directory named ahead of
    /// the repository's overrides it without editing a checkout. A
    /// directory that is not there is not an error — a machine with no
    /// manifests has no capabilities, which is a state this module is
    /// built to report.
    #[must_use]
    pub fn open(dirs: &[PathBuf]) -> Self {
        let mut merged = Registry::default();
        for dir in dirs {
            let Ok(registry) = Registry::read(dir) else {
                continue;
            };
            for manifest in registry.manifests {
                if merged.get(&manifest.slug).is_none() {
                    merged.manifests.push(manifest);
                }
            }
            merged.refused.extend(registry.refused);
        }
        merged.manifests.sort_by(|a, b| a.slug.cmp(&b.slug));
        merged
    }

    /// The manifests, in slug order.
    #[must_use]
    pub fn manifests(&self) -> &[Manifest] {
        &self.manifests
    }

    /// One manifest by slug.
    #[must_use]
    pub fn get(&self, slug: &str) -> Option<&Manifest> {
        self.manifests.iter().find(|manifest| manifest.slug == slug)
    }

    /// The files that were not manifests, each with its reason.
    #[must_use]
    pub fn refused(&self) -> &[(String, String)] {
        &self.refused
    }

    /// Probes every manifest against one workspace.
    #[must_use]
    pub fn probe_all(&self, workspace: &Path) -> Vec<Found> {
        self.manifests
            .iter()
            .map(|manifest| manifest.probe(workspace))
            .collect()
    }
}

/// The capabilities a host may offer, out of what a probe found.
///
/// This is the whole reason the probe answers three states. The option set
/// for the program-selection decision is built from what is here, so an
/// absent capability drops out of the list rather than becoming a route
/// that fails, and an operator without Devin is offered a shorter list
/// rather than a broken one.
#[must_use]
pub fn options(found: &[Found]) -> Vec<&Found> {
    found.iter().filter(|one| one.available()).collect()
}

/// Where a host looks for manifests, in order.
///
/// `CODER_CAPABILITY_DIR` first, then the repository's own `capabilities/`
/// directory, then the operator's `~/.openagents/capabilities`. The
/// repository comes before the home directory because a checkout that
/// carries a manifest describes the executors its own tasks name.
#[must_use]
pub fn search(repository: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = env::var_os(DIR_ENV).filter(|dir| !dir.is_empty()) {
        dirs.push(PathBuf::from(dir));
    }
    if let Some(root) = repository {
        dirs.push(root.join("capabilities"));
    }
    if let Some(home) = env::var_os("HOME").filter(|home| !home.is_empty()) {
        dirs.push(PathBuf::from(home).join(".openagents").join("capabilities"));
    }
    dirs
}

/// The directories the probe looks in for an executable, in order.
///
/// `CODER_CAPABILITY_PATH`, then `PATH`, then the directories a login shell
/// usually adds. `PATH` is a source of candidate directories and not the
/// answer: what the probe reports, and what a delegation later runs, is
/// always an absolute path.
#[must_use]
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push = |dir: PathBuf| {
        if !dir.as_os_str().is_empty() && seen.insert(dir.clone()) {
            dirs.push(dir);
        }
    };
    for var in [PATH_ENV, "PATH"] {
        for dir in split_path(env::var_os(var)) {
            push(dir);
        }
    }
    if let Some(home) = env::var_os("HOME").filter(|home| !home.is_empty()) {
        let home = PathBuf::from(home);
        for dir in HOME_DIRS {
            push(home.join(dir));
        }
    }
    for dir in FALLBACK_DIRS {
        push(PathBuf::from(dir));
    }
    dirs
}

/// The absolute path of `binary` in the first of `dirs` that holds it as an
/// executable file, or `None`.
#[must_use]
pub fn resolve(binary: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    // A manifest that names a path rather than a bare name is taken at its
    // word, so a host is never forced to plant a binary in a search
    // directory to reach it.
    let named = Path::new(binary);
    if named.components().count() > 1 {
        return executable(named).then(|| absolute(named));
    }
    dirs.iter()
        .map(|dir| dir.join(binary))
        .find(|candidate| executable(candidate))
        .map(|candidate| absolute(&candidate))
}

fn split_path(value: Option<OsString>) -> Vec<PathBuf> {
    value
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default()
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    env::current_dir()
        .map(|dir| dir.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn executable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.is_file())
        .unwrap_or(false)
}

/// Runs one argv, with the resolved path in place of its first element, and
/// returns everything it printed.
///
/// Standard error joins standard output because a refusal is usually
/// printed there, and a probe that read only standard output would call a
/// refusing executor present.
///
/// # Errors
///
/// Returns a sentence when the process could not be started.
fn run(path: &Path, argv: &[String], workspace: &Path) -> Result<String, String> {
    let output = Command::new(path)
        .args(&argv[1..])
        .current_dir(workspace)
        .output()
        .map_err(|error| error.to_string())?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&stderr);
    }
    Ok(text)
}

fn first_line(text: &str) -> &str {
    text.lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim()
}

/// The first token of `line` that reads as a dotted version.
///
/// `devin 3000.10.31 (b98cc431)` gives `3000.10.31`: the commit is not a
/// version and a probe that reported it as one would compare two machines
/// by their build hashes.
#[must_use]
pub fn version_in(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|token| {
        let token = token.trim_start_matches(['v', 'V']);
        let core = token.trim_matches(|c: char| !c.is_ascii_digit());
        let dotted = core.contains('.')
            && core.starts_with(|c: char| c.is_ascii_digit())
            && core.chars().all(|c| c.is_ascii_digit() || c == '.');
        dotted.then(|| core.to_string())
    })
}

/// Whether `slug` matches the NIP-CAP slug grammar.
#[must_use]
pub fn is_slug(slug: &str) -> bool {
    let mut chars = slug.chars();
    let first = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    first
        && slug.len() <= 64
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A manifest whose executor is `/bin/sh`, so a test can drive all
    /// three states against a real process rather than a stub.
    fn shim(probe: Option<&str>) -> Manifest {
        Manifest {
            v: 1,
            slug: "shim".to_string(),
            name: "A shim".to_string(),
            summary: String::new(),
            transport: "subprocess".to_string(),
            detect: Detect {
                binary: "sh".to_string(),
                version: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo shim 1.2.3 build-abc1234".to_string(),
                ],
                probe: None,
            },
            enforces: vec!["minutes".to_string()],
            cannot_enforce: vec!["tool_set".to_string()],
            sees_repository: true,
            concurrent_max: Some(2),
            cost: "local".to_string(),
            isolation: vec!["directory".to_string()],
            invoke: vec!["sh".to_string(), "-c".to_string()],
            workspace_probe: probe.map(|script| WorkspaceProbe {
                argv: vec!["sh".to_string(), "-c".to_string(), script.to_string()],
                note: String::new(),
            }),
            refuses: vec![Refusal {
                name: "untrusted_workspace".to_string(),
                matches: "Refusing to run in an untrusted workspace".to_string(),
                explanation: "It declines a directory nobody trusted.".to_string(),
            }],
        }
    }

    /// The same manifest naming an executable nothing has.
    fn nowhere() -> Manifest {
        let binary = "no-such-executor-openagents".to_string();
        let mut manifest = shim(None);
        manifest.detect.version = vec![binary.clone(), "--version".to_string()];
        manifest.detect.binary = binary;
        manifest
    }

    #[test]
    fn a_resolved_executor_with_a_version_is_present() {
        let found = shim(None).probe(Path::new("/"));
        let Presence::Present { version, path, .. } = &found.presence else {
            panic!("expected present, got {:?}", found.presence);
        };
        assert!(found.available());
        assert_eq!(found.presence.state(), "present");
        assert_eq!(version, "1.2.3");
        assert!(path.is_absolute(), "the probe resolves an absolute path");
    }

    #[test]
    fn a_missing_executor_is_absent_and_not_a_failure() {
        let found = nowhere().probe(Path::new("/"));

        assert!(!found.available());
        assert_eq!(found.presence.state(), "absent");
        assert!(!found.presence.installed());
        assert_eq!(
            found.call().outcome,
            Outcome::Completed,
            "absence is an answer the host asked for, not a failed call"
        );
        let Presence::Absent { looked_in, .. } = &found.presence else {
            panic!("expected absent, got {:?}", found.presence);
        };
        assert!(!looked_in.is_empty(), "the probe says where it looked");
    }

    /// The state the golden recorded and a present-or-absent probe cannot
    /// represent: detected, versioned, and declining this directory.
    #[test]
    fn a_refusing_executor_is_present_and_unavailable() {
        let script =
            "echo 'Error: Refusing to run in an untrusted workspace: /private/tmp' >&2; exit 1";
        let found = shim(Some(script)).probe(Path::new("/"));

        assert_eq!(found.presence.state(), "present_unavailable");
        assert!(found.presence.installed(), "it is installed");
        assert!(!found.available(), "and it is not a route");
        let Presence::Unavailable {
            refusal,
            version,
            detail,
            ..
        } = &found.presence
        else {
            panic!("expected present and unavailable, got {:?}", found.presence);
        };
        assert_eq!(refusal, "untrusted_workspace");
        assert_eq!(version, "1.2.3", "a refusing executor still has a version");
        assert!(detail.contains("untrusted workspace"));
    }

    #[test]
    fn a_workspace_probe_that_says_nothing_leaves_it_present() {
        let found = shim(Some("echo ready")).probe(Path::new("/"));
        assert_eq!(found.presence.state(), "present");
    }

    #[test]
    fn options_drop_everything_that_is_not_a_route() {
        let present = shim(None).probe(Path::new("/"));
        let refusing =
            shim(Some("echo Refusing to run in an untrusted workspace >&2")).probe(Path::new("/"));
        let absent = nowhere().probe(Path::new("/"));

        let found = vec![present, refusing, absent];
        assert_eq!(options(&found).len(), 1, "one of the three is a route");
    }

    #[test]
    fn the_probe_records_the_golden_shape() {
        let found = shim(None).probe(Path::new("/"));
        let call = found.call();
        assert_eq!(call.name, PROBE_CALL);
        assert_eq!(call.arguments["detect"]["binary"], json!("sh"));
        assert_eq!(call.extra["capability"], json!("shim"));
        assert_eq!(call.extra["present"], json!(true));
        assert_eq!(call.extra["enforces"], json!(["minutes"]));
        assert_eq!(call.extra["cannot_enforce"], json!(["tool_set"]));
        assert_eq!(call.extra["sees_repository"], json!(true));
        assert_eq!(call.extra["refuses"], json!(["untrusted_workspace"]));
        assert!(call.output.contains(" at /"), "the output names the path");
    }

    #[test]
    fn a_refused_delegation_names_the_bounds_the_executor_would_ignore() {
        let manifest = shim(None);
        assert!(manifest.ignored_bounds(&["minutes".to_string()]).is_empty());
        assert_eq!(
            manifest.ignored_bounds(&["minutes".to_string(), "tool_set".to_string()]),
            vec!["tool_set".to_string()]
        );
    }

    #[test]
    fn a_manifest_this_version_does_not_read_is_refused() {
        let mut manifest = shim(None);
        manifest.v = 2;
        assert!(manifest.validate().is_err());
        manifest.v = 1;
        manifest.slug = "Not A Slug".to_string();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn versions_parse_off_the_line_the_executor_printed() {
        assert_eq!(
            version_in("devin 3000.10.31 (b98cc431)").as_deref(),
            Some("3000.10.31")
        );
        assert_eq!(version_in("codex-cli 0.5.1-beta").as_deref(), Some("0.5.1"));
        assert_eq!(version_in("v1.2.3").as_deref(), Some("1.2.3"));
        assert_eq!(version_in("no version here"), None);
    }

    #[test]
    fn the_repository_manifests_load() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../capabilities");
        let registry = Registry::read(&dir).expect("the repository carries a capabilities dir");
        assert!(registry.refused().is_empty(), "{:?}", registry.refused());
        let devin = registry
            .get("devin-local")
            .expect("devin-local is declared");
        assert_eq!(devin.detect.binary, "devin");
        assert_eq!(devin.enforces, ["max_turns", "minutes", "model"]);
        assert_eq!(
            devin.cannot_enforce,
            ["tool_set", "role", "budget_cents", "effort"]
        );
        assert!(devin.sees_repository);
        assert_eq!(devin.invoke, ["devin", "-p", "--"]);
        assert_eq!(devin.refuses[0].name, "untrusted_workspace");
        assert!(
            devin.workspace_probe.is_some(),
            "it can be asked about a directory"
        );
    }

    /// The reproduction the golden described, run against whatever this
    /// machine actually has.
    ///
    /// A directory made a moment ago has never been trusted interactively,
    /// so the two honest answers are absent, on a machine without the
    /// executor, and present-and-unavailable on one with it. Present is the
    /// answer this test exists to catch.
    #[test]
    fn the_real_executor_does_not_claim_a_fresh_directory() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../capabilities");
        let registry = Registry::read(&dir).unwrap();
        let devin = registry.get("devin-local").unwrap();
        let workspace = tempfile::tempdir().expect("a directory to ask about");

        let found = devin.probe(workspace.path());
        match &found.presence {
            Presence::Absent { .. } => {}
            Presence::Unavailable { refusal, .. } => {
                assert_eq!(refusal, "untrusted_workspace");
            }
            Presence::Present { .. } => {
                panic!("the executor claimed a directory nobody has trusted")
            }
        }
    }
}
