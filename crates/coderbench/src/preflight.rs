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
//! about whether this computer has one. The manifest, the registry, the
//! approval, and the bounded probe are the shared `capability` crate's —
//! the same contract `coder::capability` serves, so the two readers
//! cannot drift. Reading is inert and probing runs only under an
//! approval the operator recorded; a manifest nobody approved is
//! unmet with the approval path named, not silently run.
//!
//! This is the harness checking its own preconditions. Coder's own probe
//! is a step in the run's trace; this stays what it is: the reason the
//! run was worth starting.
//!
//! A `relay` capability is the one requirement this machine cannot answer
//! for: the executor is a worker on the far side of a relay, and the
//! approval and the binary are the worker's. What the harness can check is
//! that the run is told where to ask — `CODER_RELAY` and `CODER_WORKER` —
//! and the run's own `capability_probe` check is what says whether the
//! worker answered. `CODER_DELEGATE` names the capability the run will
//! delegate through in place of the one the task requires, and the
//! requirement follows it, so a task written for `devin-local` runs
//! through `devin-relay` on a host with no Devin CLI at all.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use capability::{Presence, Registry, SourceDir, Trust};

use crate::{Task, capabilities_dir, drive};

/// How long a `detect` command may take before the capability counts as
/// unresolvable. A probe is a version string; one that blocks is a broken
/// executor rather than a slow one.
const DETECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The variable that names a capability registry, which `crates/coder`
/// reads under the same name. One knob, both readers.
pub const CAPABILITY_DIR: &str = capability::DIR_ENV;

/// The variable that names the capability a run delegates through, which
/// `crates/coder` reads under the same name. When set, it replaces the
/// capability the task requires in the preflight, because the run will
/// not use the one the task named.
pub const DELEGATE_VAR: &str = "CODER_DELEGATE";

/// The relay a `relay` capability is asked over, and the worker it asks.
/// The names `crates/coder` reads.
pub const RELAY_VAR: &str = "CODER_RELAY";
pub const WORKER_VAR: &str = "CODER_WORKER";

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
/// run would happen, then what it would reach — under the operator's
/// trust, so a capability probe runs only under a recorded approval.
///
/// Every requirement is reported, met or not, because a run that starts is
/// a run somebody will read the faults of, and the faults mean one thing
/// at the base commit and another thing anywhere else.
#[must_use]
pub fn check(task: &Task, repository: &Path) -> Vec<Checked> {
    check_with(task, repository, &Trust::operator())
}

/// The same check under a trust the caller chose — a test's own store,
/// or [`Trust::empty`] for a check that must prove it runs nothing.
#[must_use]
pub fn check_with(task: &Task, repository: &Path, trust: &Trust) -> Vec<Checked> {
    let mut checked = Vec::new();
    if !task.requires.repository.is_empty() {
        checked.push(same_repository(&task.requires.repository, repository));
    }
    if !task.requires.base.is_empty() {
        checked.push(at_base(&task.requires.base, repository));
        checked.push(unmodified(repository));
    }
    let delegate = std::env::var(DELEGATE_VAR)
        .ok()
        .filter(|slug| !slug.is_empty());
    for slug in &task.requires.capabilities {
        match &delegate {
            Some(chosen) if chosen != slug => {
                let mut substituted = capability(
                    chosen,
                    repository,
                    &task.requires.capabilities_refuse,
                    trust,
                );
                substituted.requirement = format!(
                    "{} (in place of {slug}, by {DELEGATE_VAR})",
                    substituted.requirement
                );
                checked.push(substituted);
            }
            _ => checked.push(capability(
                slug,
                repository,
                &task.requires.capabilities_refuse,
                trust,
            )),
        }
    }
    checked
}

/// The requirements that did not hold.
#[must_use]
pub fn unmet(checked: &[Checked]) -> Vec<&Checked> {
    checked.iter().filter(|one| !one.met).collect()
}

/// Where a manifest is read from, in order.
///
/// The same order `capability::search` takes, with this workspace's own
/// registry last. A checkout pinned to a commit from before the registry
/// existed still needs the harness to know what `devin-local` is. The
/// bundled registry is repository data, like any checkout's.
#[must_use]
pub fn registries(repository: Option<&Path>) -> Vec<SourceDir> {
    let mut directories = capability::search(repository);
    directories.push(SourceDir::repository(capabilities_dir()));
    directories
}

/// Resolves `binary` to a file that can be run, through the same search
/// `capability` resolves probes with: `CODER_CAPABILITY_PATH`, `PATH`,
/// then the directories an interactive shell usually adds.
#[must_use]
pub fn resolve(binary: &str) -> Option<PathBuf> {
    capability::resolve(binary, &capability::search_dirs())
}

/// Whether the capability is installed here, and at what version.
///
/// The probe is the shared contract's: the manifest comes from the same
/// registry order, the argv runs only under an approval the operator
/// recorded, and the answer is typed — `unprobed` and `unknown` are
/// unmet with their reasons, not silent passes or silent failures.
fn capability(
    slug: &str,
    repository: &Path,
    refuses: &BTreeMap<String, Vec<String>>,
    trust: &Trust,
) -> Checked {
    let refused = refuses.get(slug).filter(|what| !what.is_empty());
    let requirement = match refused {
        Some(what) => format!("capability {slug}, which refuses {}", what.join(", ")),
        None => format!("capability {slug}"),
    };
    let dirs = registries(Some(repository));
    let registry = Registry::open(&dirs);
    let Some(entry) = registry.entry(slug) else {
        return Checked {
            requirement,
            found: format!(
                "nothing describes it — no {slug}.json under {}",
                dirs.iter()
                    .map(|dir| dir.path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            met: false,
        };
    };
    if entry.manifest.transport == capability::RELAY {
        return relay_capability(requirement);
    }
    let found = entry.detect(repository, trust);
    let met = matches!(found.presence, Presence::Present { .. });
    Checked {
        requirement,
        found: match &found.presence {
            Presence::Present { report, path, .. } => {
                format!("{report} at {}", path.display())
            }
            _ => found.output(),
        },
        met,
    }
}

/// Whether a `relay` capability can be asked about at all.
///
/// Nothing here can run the executor: the worker holds the binary and the
/// approval. The requirement is met when the run knows which relay and
/// which worker to ask; whether the worker answers is the run's own
/// `capability_probe` check, recorded in the trace where the grade reads
/// it. A missing variable is named so the operator sets it rather than
/// reading a run that never reached a worker.
fn relay_capability(requirement: String) -> Checked {
    let relay = std::env::var(RELAY_VAR)
        .ok()
        .filter(|value| !value.is_empty());
    let worker = std::env::var(WORKER_VAR)
        .ok()
        .filter(|value| !value.is_empty());
    match (relay, worker) {
        (Some(relay), Some(worker)) => Checked {
            requirement,
            found: format!(
                "a worker {worker} over {relay}; the run's capability_probe check says whether it answers"
            ),
            met: true,
        },
        (relay, worker) => {
            let mut missing = Vec::new();
            if relay.is_none() {
                missing.push(RELAY_VAR);
            }
            if worker.is_none() {
                missing.push(WORKER_VAR);
            }
            Checked {
                requirement,
                found: format!(
                    "a relay capability with nothing to ask — set {}",
                    missing.join(" and ")
                ),
                met: false,
            }
        }
    }
}

/// Whether this checkout is the repository the task names.
///
/// The configured `remote.origin.url` is what the checkout says it is.
/// `git remote get-url` would report the URL after `insteadOf` rewrites,
/// so a host whose global configuration rewrites `github.com` through a
/// proxy would refuse a checkout that matches.
fn same_repository(wanted: &str, repository: &Path) -> Checked {
    let requirement = format!("repository {wanted}");
    match git(repository, &["config", "--get", "remote.origin.url"]) {
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

    /// A host-wide `insteadOf` rewrite changes what `remote get-url`
    /// reports, not what the checkout's own configuration names. The
    /// check reads the configured URL, so a matching checkout still
    /// passes — and a different repository still refuses.
    #[test]
    fn a_rewritten_remote_still_names_the_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let repository = dir.path().join("checkout");
        std::fs::create_dir(&repository).unwrap();
        git(&repository, &["init"]).unwrap();
        git(
            &repository,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/OpenAgentsInc/openagents.git",
            ],
        )
        .unwrap();
        // The rewrite lives in the checkout's own configuration, which
        // `remote get-url` applies exactly as it would a global one, so the
        // test touches no process environment that other tests share.
        git(
            &repository,
            &[
                "config",
                "url.https://git-manager.devin.ai/proxy/github.com/.insteadOf",
                "https://github.com/",
            ],
        )
        .unwrap();
        let rewritten = git(&repository, &["remote", "get-url", "origin"]).unwrap();
        let same = same_repository("https://github.com/OpenAgentsInc/openagents", &repository);
        let different = same_repository("https://github.com/OpenAgentsInc/coder", &repository);

        assert!(
            rewritten.starts_with("https://git-manager.devin.ai/proxy/"),
            "the rewrite has to be in force for the check to mean anything: {rewritten}"
        );
        assert!(same.met, "{}", same.found);
        assert!(!different.met, "{}", different.found);
    }

    /// The workspace registry answers for the executor the task names,
    /// even when the checkout being measured carries no registry of its
    /// own.
    #[test]
    fn the_registry_answers_for_devin() {
        let registry = Registry::open(&registries(None));
        let manifest = registry
            .get("devin-local")
            .expect("the registry describes devin-local");
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
            &Trust::empty(),
        );
        assert!(!checked.met);
        assert!(
            checked.found.contains("nothing-describes-this.json"),
            "{}",
            checked.found
        );
    }

    /// A manifest nobody approved is unmet and its argv never runs — the
    /// approval path is named in the reason, and the marker proves the
    /// executable stayed inert.
    #[test]
    fn an_unapproved_manifest_is_unprobed_and_runs_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let registry_dir = dir.path().join("capabilities");
        std::fs::create_dir_all(&registry_dir).unwrap();
        let marker = dir.path().join("ran");
        std::fs::write(
            registry_dir.join("side-effect.json"),
            serde_json::to_string(&capability::executor_document(
                "side-effect",
                "sh",
                vec![
                    "sh".into(),
                    "-c".into(),
                    format!("touch {}; echo side-effect 1.0.0", marker.display()),
                ],
                serde_json::json!({"name": "A"}),
            ))
            .unwrap(),
        )
        .unwrap();

        let checked = capability("side-effect", dir.path(), &BTreeMap::new(), &Trust::empty());
        assert!(!checked.met);
        assert!(
            checked.found.contains("capability-trust approve"),
            "{}",
            checked.found
        );
        assert!(
            !marker.exists(),
            "an unapproved manifest's argv must not run"
        );
    }

    /// The production check decides under the operator's store: a
    /// checkout that ships a `capability-trust.json` of its own — even a
    /// real record copied in — does not approve its own manifest, and
    /// nothing runs.
    #[test]
    fn a_checkout_cannot_approve_its_own_capability() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let registry_dir = dir.path().join("capabilities");
        std::fs::create_dir_all(&registry_dir).unwrap();
        let marker = dir.path().join("ran");
        std::fs::write(
            registry_dir.join("side-effect.json"),
            serde_json::to_string(&capability::executor_document(
                "side-effect",
                "sh",
                vec![
                    "sh".into(),
                    "-c".into(),
                    format!("touch {}; echo side-effect 1.0.0", marker.display()),
                ],
                serde_json::json!({"name": "A"}),
            ))
            .unwrap(),
        )
        .unwrap();

        let mut approved = Trust::load(&outside.path().join("capability-trust.json")).unwrap();
        approved
            .approve(Some(dir.path()), "side-effect", &[])
            .unwrap();
        std::fs::copy(
            outside.path().join("capability-trust.json"),
            dir.path().join("capability-trust.json"),
        )
        .unwrap();

        let checked = capability(
            "side-effect",
            dir.path(),
            &BTreeMap::new(),
            &Trust::operator(),
        );
        assert!(!checked.met, "{}", checked.found);
        assert!(!marker.exists(), "the checkout's own record ran nothing");
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
