//! What has to hold before a run starts, and what the machine answers.
//!
//! A task states its requirements so a harness can refuse rather than
//! produce faults that are about the environment. The two that matter are
//! the checkout and the executors:
//!
//! - **The checkout.** Delegates read the working copy, not the commit you
//!   think you are on. A run one rename behind the task answers honestly
//!   and wrongly, and the faults it produces are about the rename.
//! - **The executors.** A capability that is not installed is not a fault
//!   in the agent. It is a machine that cannot run this task.
//!
//! Presence is local fact, as [NIP-CAP](../../../nips/openagents/NIP-CAP.md)
//! puts it: a manifest describes how to drive an executor and says nothing
//! about whether this computer has one. So a manifest is read from disk and
//! its `detect` argv is run here.
//!
//! This is the harness checking its own preconditions, not the capability
//! probe Coder owes its own runs. When Coder grows one, a run's probe
//! becomes a step in the trace and this stays what it is: the reason the
//! run was worth starting.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use crate::{Task, capabilities_dir, drive};

/// How long a `detect` command may take before the capability counts as
/// unresolvable. A probe is a version string; one that blocks is a broken
/// executor rather than a slow one.
const DETECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The variable that names a capability registry, which `crates/coder`
/// reads under the same name. One knob, both readers.
pub const CAPABILITY_DIR: &str = "CODER_CAPABILITY_DIR";

/// Directories to look in when `PATH` does not resolve a binary.
///
/// The first recording of `devin-fan-out-six` failed six delegations with
/// `command not found: devin` on a machine that had Devin installed: the
/// binary was on the operator's interactive `PATH` and not on the one a
/// spawned subshell inherits. A probe that reported that capability absent
/// would have been wrong about the machine.
const ALSO_LOOK_IN: [&str; 4] = [
    "~/.local/bin",
    "~/bin",
    "/opt/homebrew/bin",
    "/usr/local/bin",
];

/// One requirement, and what the machine said about it.
#[derive(Clone, Debug)]
pub struct Checked {
    /// What the task asks for, in the task's terms.
    pub requirement: String,
    /// What is actually here.
    pub found: String,
    pub met: bool,
}

/// Everything a task requires, in the order a reader wants it: where the
/// run would happen, then what it would reach.
///
/// Every requirement is reported, met or not, because a run that starts is
/// a run somebody will read the faults of, and the faults mean one thing
/// at the base commit and another thing anywhere else.
#[must_use]
pub fn check(task: &Task, repository: &Path) -> Vec<Checked> {
    let mut checked = Vec::new();
    if !task.requires.repository.is_empty() {
        checked.push(same_repository(&task.requires.repository, repository));
    }
    if !task.requires.base.is_empty() {
        checked.push(at_base(&task.requires.base, repository));
        checked.push(unmodified(repository));
    }
    for slug in &task.requires.capabilities {
        checked.push(capability(
            slug,
            repository,
            &task.requires.capabilities_refuse,
        ));
    }
    checked
}

/// The requirements that did not hold.
#[must_use]
pub fn unmet(checked: &[Checked]) -> Vec<&Checked> {
    checked.iter().filter(|one| !one.met).collect()
}

/// A capability manifest, as [NIP-CAP](../../../nips/openagents/NIP-CAP.md)
/// kind `30180` carries it.
///
/// Only `detect` is read. The rest of the body says how to drive an
/// executor, which is `crates/coder`'s business rather than this crate's:
/// the harness asks whether the machine has the executor, not what to do
/// with it.
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub detect: Detect,
}

/// What a host runs to decide an executor is here.
#[derive(Clone, Debug, Deserialize)]
pub struct Detect {
    /// The executable to resolve.
    pub binary: String,
    /// The argv that reports the version.
    ///
    /// An argv, never a shell string. A manifest is untrusted input, and a
    /// manifest that could name a shell command would be a way to run one.
    #[serde(default)]
    pub version: Vec<String>,
}

impl Manifest {
    /// Reads the manifest for a capability slug, from the first registry
    /// that holds one.
    ///
    /// # Errors
    ///
    /// Returns an error when no registry holds the slug, or the manifest
    /// does not parse. An unknown slug is an error rather than an absent
    /// capability: a task asking for something nothing describes is a task
    /// nobody can say is runnable.
    pub fn load(slug: &str, repository: Option<&Path>) -> Result<Self, String> {
        let mut looked = Vec::new();
        for directory in registries(repository) {
            let path = directory.join(format!("{slug}.json"));
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    return serde_json::from_str(&text)
                        .map_err(|error| format!("{}: {error}", path.display()));
                }
                Err(_) => looked.push(directory.display().to_string()),
            }
        }
        Err(format!("no {slug}.json in {}", looked.join(", ")))
    }
}

/// Where a manifest is read from, in order.
///
/// The same order `coder::capability::search` takes, with this workspace's
/// own registry last. A checkout pinned to a commit from before the
/// registry existed still needs the harness to know what `devin-local` is.
#[must_use]
pub fn registries(repository: Option<&Path>) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(named) = std::env::var_os(CAPABILITY_DIR).filter(|named| !named.is_empty()) {
        directories.push(PathBuf::from(named));
    }
    if let Some(repository) = repository {
        directories.push(repository.join("capabilities"));
    }
    if let Ok(home) = std::env::var("HOME") {
        directories.push(PathBuf::from(home).join(".openagents").join("capabilities"));
    }
    directories.push(capabilities_dir());
    directories
}

/// Resolves `binary` to a file that can be run, on `PATH` first and then in
/// the directories an interactive shell usually adds.
#[must_use]
pub fn resolve(binary: &str) -> Option<PathBuf> {
    let named = Path::new(binary);
    if named.components().count() > 1 {
        return runnable(named).then(|| named.to_path_buf());
    }
    let path = std::env::var("PATH").unwrap_or_default();
    let directories = path
        .split(':')
        .map(PathBuf::from)
        .chain(ALSO_LOOK_IN.iter().map(|directory| expand(directory)));
    for directory in directories {
        let candidate = directory.join(binary);
        if runnable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Whether a path is a file this user can run.
fn runnable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    true
}

/// Expands a leading `~` against the home directory.
fn expand(directory: &str) -> PathBuf {
    match directory.strip_prefix("~/") {
        Some(rest) => match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home).join(rest),
            Err(_) => PathBuf::from(directory),
        },
        None => PathBuf::from(directory),
    }
}

/// Whether the capability is installed here, and at what version.
fn capability(slug: &str, repository: &Path, refuses: &BTreeMap<String, Vec<String>>) -> Checked {
    let refused = refuses.get(slug).filter(|what| !what.is_empty());
    let requirement = match refused {
        Some(what) => format!("capability {slug}, which refuses {}", what.join(", ")),
        None => format!("capability {slug}"),
    };
    let manifest = match Manifest::load(slug, Some(repository)) {
        Ok(manifest) => manifest,
        Err(error) => {
            return Checked {
                requirement,
                found: format!("nothing describes it — {error}"),
                met: false,
            };
        }
    };
    let Some(resolved) = resolve(&manifest.detect.binary) else {
        return Checked {
            requirement,
            found: format!("{} is not installed here", manifest.detect.binary),
            met: false,
        };
    };
    let Some((first, rest)) = manifest.detect.version.split_first() else {
        return Checked {
            requirement,
            found: format!("{}", resolved.display()),
            met: true,
        };
    };
    // The manifest names the binary and `PATH` may not resolve it, so the
    // resolved file is what runs. The rest of the argv is the manifest's.
    let _ = first;
    let mut command = Command::new(&resolved);
    command.args(rest);
    match drive::output(command, DETECT_TIMEOUT) {
        Ok(reported) if reported.code == Some(0) => Checked {
            requirement,
            found: format!("{} at {}", reported.out.trim(), resolved.display()),
            met: true,
        },
        Ok(reported) => Checked {
            requirement,
            found: format!(
                "{} is installed at {} and {} exited {}",
                manifest.detect.binary,
                resolved.display(),
                manifest.detect.version.join(" "),
                reported
                    .code
                    .map_or_else(|| "on a signal".to_string(), |code| code.to_string())
            ),
            met: false,
        },
        Err(error) => Checked {
            requirement,
            found: format!("{} could not be run — {error}", resolved.display()),
            met: false,
        },
    }
}

/// Whether this checkout is the repository the task names.
fn same_repository(wanted: &str, repository: &Path) -> Checked {
    let requirement = format!("repository {wanted}");
    match git(repository, &["remote", "get-url", "origin"]) {
        Ok(origin) => {
            let met = bare(&origin) == bare(wanted);
            Checked {
                requirement,
                found: if met {
                    origin
                } else {
                    format!("origin is {origin}")
                },
                met,
            }
        }
        Err(error) => Checked {
            requirement,
            found: error,
            met: false,
        },
    }
}

/// A remote URL without the parts that differ between ways of cloning the
/// same repository, so an SSH remote and an HTTPS one compare equal.
fn bare(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    let url = url.split_once("://").map_or(url, |(_, rest)| rest);
    let url = url.split_once('@').map_or(url, |(_, rest)| rest);
    // An SSH remote writes `host:owner/name` where an HTTPS one writes
    // `host/owner/name`.
    url.replacen(':', "/", 1).to_lowercase()
}

/// Whether the checkout is at the commit the task pins.
fn at_base(base: &str, repository: &Path) -> Checked {
    let requirement = format!("repository at {}", short(base));
    match git(repository, &["rev-parse", "HEAD"]) {
        Ok(head) => {
            let met = head.starts_with(base) || base.starts_with(&head);
            Checked {
                requirement,
                found: if met {
                    short(&head)
                } else {
                    format!("{} is checked out", short(&head))
                },
                met,
            }
        }
        Err(error) => Checked {
            requirement,
            found: error,
            met: false,
        },
    }
}

/// Whether the working copy is the commit rather than the commit plus
/// somebody's afternoon.
///
/// A delegate reads files, and an edited file is not the base commit
/// however the commit reads.
fn unmodified(repository: &Path) -> Checked {
    let requirement = "no uncommitted changes".to_string();
    match git(repository, &["status", "--porcelain"]) {
        Ok(status) if status.is_empty() => Checked {
            requirement,
            found: "the working copy is clean".to_string(),
            met: true,
        },
        Ok(status) => {
            let changed = status.lines().count();
            Checked {
                requirement,
                found: format!(
                    "{changed} changed {}, starting with {}",
                    if changed == 1 { "file" } else { "files" },
                    status.lines().next().unwrap_or_default().trim()
                ),
                met: false,
            }
        }
        Err(error) => Checked {
            requirement,
            found: error,
            met: false,
        },
    }
}

/// What Git says about the checkout right now, one line per path.
///
/// Read once before the run and once after, and the difference is what the
/// run wrote. That is the independent observation a task forbidding writes
/// needs: a delegate that wrote a file and did not mention it leaves no
/// `wrote` field behind, and a grade that reads only the trace would call
/// that silence proof.
///
/// The comparison is what Git tracks and what it would show as untracked,
/// so a write into an ignored path is outside it. A checkout is the unit
/// here because it is the thing the task pins and the thing preflight
/// already requires to be clean.
///
/// # Errors
///
/// Returns an error when the directory is not a checkout this can read.
/// Unknown is the answer then, rather than an empty list, which would read
/// as "nothing changed".
pub fn worktree(repository: &Path) -> Result<Vec<String>, String> {
    let status = git(
        repository,
        &["status", "--porcelain", "--untracked-files=all"],
    )?;
    Ok(status
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect())
}

/// The paths that differ between two readings of a checkout.
///
/// Both directions count. A run that deleted an untracked file changed the
/// workspace as surely as one that added a file.
#[must_use]
pub fn changed(before: &[String], after: &[String]) -> Vec<String> {
    let mut paths: Vec<String> = after
        .iter()
        .filter(|line| !before.contains(line))
        .chain(before.iter().filter(|line| !after.contains(line)))
        .map(|line| named(line))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// The path a porcelain status line names, without its two status columns.
fn named(line: &str) -> String {
    line.get(3..).unwrap_or(line).trim().to_string()
}

/// The first twelve characters of a commit, which is what a person reads.
fn short(commit: &str) -> String {
    commit.chars().take(12).collect()
}

/// Runs one read-only Git command in `repository` and returns its output.
fn git(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repository).args(arguments);
    let reported = drive::output(command, DETECT_TIMEOUT)
        .map_err(|error| format!("git {} — {error}", arguments.join(" ")))?;
    if reported.code == Some(0) {
        Ok(reported.out.trim().to_string())
    } else {
        Err(format!(
            "{} is not a checkout this can read — {}",
            repository.display(),
            reported.err.trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two ways of naming one repository compare equal, and a different
    /// repository does not.
    #[test]
    fn a_remote_compares_by_what_it_names() {
        let https = "https://github.com/OpenAgentsInc/openagents.git";
        for same in [
            "git@github.com:OpenAgentsInc/openagents.git",
            "https://github.com/OpenAgentsInc/openagents",
            "ssh://git@github.com/OpenAgentsInc/openagents.git",
        ] {
            assert_eq!(bare(same), bare(https), "{same}");
        }
        assert_ne!(bare("git@github.com:OpenAgentsInc/coder.git"), bare(https));
    }

    /// The workspace registry answers for the executor the task names,
    /// even when the checkout being measured carries no registry of its
    /// own.
    #[test]
    fn the_registry_answers_for_devin() {
        let manifest =
            Manifest::load("devin-local", None).expect("the registry describes devin-local");
        assert_eq!(manifest.detect.binary, "devin");
        assert_eq!(
            manifest.detect.version.first().map(String::as_str),
            Some("devin")
        );
    }

    /// A slug no registry describes says so, rather than passing for an
    /// absent capability.
    #[test]
    fn an_unknown_slug_is_not_a_capability() {
        let checked = capability(
            "nothing-describes-this",
            Path::new("/nowhere"),
            &BTreeMap::new(),
        );
        assert!(!checked.met);
        assert!(
            checked.found.contains("nothing-describes-this.json"),
            "{}",
            checked.found
        );
    }

    /// Something every machine has resolves, and something nothing has
    /// does not.
    #[test]
    fn resolving_finds_what_is_installed() {
        assert!(resolve("sh").is_some());
        assert!(resolve("not-a-binary-anybody-installed").is_none());
    }

    /// A file that appeared and a file that went away are both changes, and
    /// a workspace nobody touched reports none.
    #[test]
    fn a_workspace_reads_both_directions() {
        let before = vec!["?? scratch.txt".to_string(), " M docs/one.md".to_string()];
        let after = vec![" M docs/one.md".to_string(), "?? written.txt".to_string()];
        assert_eq!(changed(&before, &after), vec!["scratch.txt", "written.txt"]);
        assert!(changed(&before, &before).is_empty());
    }

    /// A directory that is not a checkout answers "unknown" rather than
    /// "nothing changed".
    #[test]
    fn a_directory_that_is_not_a_checkout_says_so() {
        let directory = tempfile::tempdir().unwrap();
        assert!(worktree(directory.path()).is_err());
    }
}
