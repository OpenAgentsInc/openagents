//! Independent checks for local Coder runs (#10232).
//!
//! A run the terminal or autostart starts had no requirements in its
//! grant, so its route ended `completed/unchecked`, never `verified`. Each
//! writing run's grant now carries one host-written suite
//! ([`requirements`]), frozen before the candidate exists: a script
//! outside the workspace, pinned by digest, that runs the commands the
//! host lists for the turn and reports a typed verdict on the candidate.
//!
//! When the run ends, its owner lists those commands ([`list`]): the
//! checks the delegate recipe froze (#10208), each of which failed before
//! the engine started, and the tests of each Cargo package the run
//! touched, as the issue flow's gate runs them. The owner then records the
//! check's intent and runs it through the task owner's independent check
//! on the exact candidate ([`complete`]), read-only, so the route ends
//! `verified` or `check_failed` on what the checks found. With nothing to
//! list, nothing runs and the route stays `unchecked`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::json;

use super::owner::Owner;
use super::{Checks, Error, Execution, Status, Store, Task, checks};

/// The check's id in the suite's plan.
pub const CHECK: &str = "local-run";
/// The suite manifest's slug.
const SLUG: &str = "coder-local-run-checks";
/// How long the listed commands may run together: the plan's bound.
const SECONDS: u64 = 3600;
/// The source a local check's lineage names: the host, not the task.
const SOURCE: &str = "coder-host-local-checks";

fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// The directory a turn's listed commands go in, beside its suite.
fn listed_dir(suite: &Path) -> PathBuf {
    suite.with_extension("checks")
}

/// The suite script: run each listed command in order inside the
/// candidate, quietly, and report `passed` only when every one passed,
/// `failed` at the first that failed, `unverifiable` with nothing listed.
fn script(listed: &Path) -> String {
    format!(
        "#!/bin/sh\n\
# A local Coder run's independent check (#10232), written by the host\n\
# before the run: the commands listed for the turn, run on the candidate.\n\
set -u\n\
dir={dir}\n\
suite=$( (sha256sum \"$0\" 2>/dev/null || shasum -a 256 \"$0\") | cut -d ' ' -f 1)\n\
unset CARGO_BUILD_JOBS RUST_TEST_THREADS\n\
emit() {{ printf '{{\"schema\":\"openagents.verification.v1\",\"suite_digest\":\"sha256:%s\",\"input_digest\":\"%s\",\"verdict\":\"%s\"}}' \"$suite\" \"$1\" \"$2\"; exit 0; }}\n\
ran=0\n\
for check in \"$dir\"/*.sh; do\n\
  [ -f \"$check\" ] || continue\n\
  ran=1\n\
  if ! /bin/sh \"$check\" >\"${{TMPDIR:-/tmp}}/check.log\" 2>&1; then\n\
    tail -c 2000 \"${{TMPDIR:-/tmp}}/check.log\" >&2\n\
    emit \"$1\" failed\n\
  fi\n\
done\n\
[ \"$ran\" = 1 ] || emit \"$1\" unverifiable\n\
emit \"$1\" passed\n",
        dir = quoted(&listed.display().to_string()),
    )
}

/// The requirements a local run's grant carries: one host-written suite
/// at `grants/<task>-<revision>.suite.sh`, with its manifest beside it,
/// pinned by digest. Unix only; `None` elsewhere.
///
/// # Errors
/// Why the suite could not be written.
#[cfg(unix)]
pub fn requirements(
    grants: &Path,
    task: &str,
    revision: u64,
) -> Result<Option<checks::Requirements>, String> {
    use crate::capability::{self, Entry, Source};
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(grants).map_err(|e| e.to_string())?;
    let program = grants.join(format!("{task}-{revision}.suite.sh"));
    std::fs::write(&program, script(&listed_dir(&program))).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let program = program.canonicalize().map_err(|e| e.to_string())?;
    // The script names its listed commands by its own canonical place.
    std::fs::write(&program, script(&listed_dir(&program))).map_err(|e| e.to_string())?;
    let suite_digest = format!("sha256:{}", capability::digest_file(&program)?);
    let manifest = grants.join(format!("{task}-{revision}.suite.json"));
    let document = capability::executor_document(
        SLUG,
        &program.display().to_string(),
        vec![program.display().to_string(), "--version".into()],
        json!({"name":"Local Coder run checks","summary":"A local Coder run's independent check",
            "invoke":[program],"isolation":["directory"]}),
    );
    std::fs::write(
        &manifest,
        serde_json::to_vec(&document).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let manifest = manifest.canonicalize().map_err(|e| e.to_string())?;
    let entry = Entry::load(&manifest, Source::Operator)?;
    let requirements = checks::Requirements {
        schema: checks::REQUIREMENTS_SCHEMA.into(),
        version: 1,
        requirements: vec![checks::Requirement {
            id: "local-checks".into(),
            statement: "The checks the recipe froze for the turn and the tests of each Cargo \
                        package the run touched pass on the candidate."
                .into(),
            checks: vec![CHECK.into()],
        }],
        plan: json!({"schema":"openagents.verification.v1","input_digest":checks::CANDIDATE,
            "seconds":SECONDS,"allow_unrestricted_reads":true,"allow_network":true,
            "checks":[{"id":CHECK,"manifest":manifest,"manifest_digest":entry.digest,
                "arguments":[checks::CANDIDATE],"seconds":SECONDS,"output_bytes":16 * 1024,
                "acceptance":{"kind":"suite","suite_digest":suite_digest,
                    "input_digest":checks::CANDIDATE}}]}),
        instruction_targets: Vec::new(),
        source_exclusions: Vec::new(),
        task_sources: vec![task.to_owned()],
        check_lineage: vec![checks::CheckLineage {
            check: CHECK.into(),
            sources: vec![SOURCE.into()],
        }],
        knowledge: Vec::new(),
    };
    requirements.validate().map_err(|e| e.to_string())?;
    Ok(Some(requirements))
}

/// No local check suite off Unix.
///
/// # Errors
/// Never.
#[cfg(not(unix))]
pub fn requirements(
    _grants: &Path,
    _task: &str,
    _revision: u64,
) -> Result<Option<checks::Requirements>, String> {
    Ok(None)
}

/// The listed-commands directory of `requirements` when they are a local
/// run's suite ([`requirements`]), else `None`.
fn ours(requirements: &checks::Requirements) -> Option<PathBuf> {
    let plan = requirements.validate().ok()?;
    let [check] = plan.checks.as_slice() else {
        return None;
    };
    if check.id != CHECK
        || requirements
            .check_lineage
            .iter()
            .any(|l| l.sources != [SOURCE])
        || check.manifest.extension().is_none_or(|e| e != "json")
    {
        return None;
    }
    Some(listed_dir(&check.manifest.with_extension("sh")))
}

/// The Cargo packages the run touched since `source_revision`: changed,
/// added, removed, and new untracked files, each under its nearest package.
#[must_use]
pub fn touched_packages(workspace: &Path, source_revision: &str) -> Vec<String> {
    let git = super::owner::GIT_PATHS
        .into_iter()
        .find(|path| Path::new(path).is_file())
        .unwrap_or(super::owner::GIT_PATHS[0]);
    let run = |arguments: &[&str]| {
        std::process::Command::new(git)
            .env_clear()
            .envs(super::owner::base_environment())
            .env("PATH", super::owner::SYSTEM_PATH)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .arg("-C")
            .arg(workspace)
            .args(arguments)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default()
    };
    let changed = run(&["diff", "--name-only", source_revision, "--"]);
    let untracked = run(&["ls-files", "--others", "--exclude-standard"]);
    let diff: String = changed
        .lines()
        .chain(untracked.lines())
        .filter(|line| !line.is_empty())
        .map(|path| format!("+++ b/{path}\n"))
        .collect();
    coder_delegate::issue::changed_packages(workspace, &diff)
}

/// List what a local run's check runs, when `requirements` are a local
/// run's suite: `frozen` (the recipe's checks) then `cargo test -p` for
/// each package [`touched_packages`] finds, each with `environment`.
/// Returns the listed directory, or `None` when there is nothing to run
/// or it could not be written.
pub(super) fn list(
    requirements: &checks::Requirements,
    workspace: &Path,
    source_revision: &str,
    frozen: &[String],
    environment: &[(String, std::ffi::OsString)],
) -> Option<PathBuf> {
    let dir = ours(requirements)?;
    let mut commands: Vec<String> = frozen
        .iter()
        .filter(|command| !command.trim().is_empty())
        .cloned()
        .collect();
    for package in touched_packages(workspace, source_revision) {
        let command = format!("cargo test -p {}", quoted(&package));
        if !commands.contains(&command) {
            commands.push(command);
        }
    }
    if commands.is_empty() {
        return None;
    }
    let prelude: String = environment
        .iter()
        .filter_map(|(name, value)| {
            let value = value.to_str()?;
            Some(format!("{name}={}\nexport {name}\n", quoted(value)))
        })
        .collect();
    // Written beside, then moved into place whole.
    let partial = dir.with_extension("checks.partial");
    let _ = std::fs::remove_dir_all(&partial);
    std::fs::create_dir_all(&partial).ok()?;
    for (index, command) in commands.iter().enumerate() {
        std::fs::write(
            partial.join(format!("{:03}.sh", index + 1)),
            format!("{prelude}{command}\n"),
        )
        .ok()?;
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&partial, &dir).ok()?;
    Some(dir)
}

/// The listed directory of `task`'s current run, when its grant carries a
/// local run's suite and the owner listed commands for it.
fn listed(task: &Task) -> Option<PathBuf> {
    let requirements = task.run.as_ref()?.admission.grant.requirements.as_ref()?;
    ours(requirements).filter(|dir| dir.is_dir())
}

/// Run the check `task`'s owner recorded the intent of when the run
/// ended ([`super::adapter::Host::finish`]), and record its report. The
/// task as it stands when there is nothing to run.
///
/// # Errors
/// Store and lock failures, or the check could not run.
pub async fn complete(directory: &Path, id: &str) -> Result<Task, Error> {
    let store = Store::open_for_owner(directory)?;
    let mut owner = None;
    // A reader may hold the lock for a moment ([`pending`]).
    for _ in 0..50 {
        match Owner::acquire(&store, id) {
            Ok(held) => {
                owner = Some(held);
                break;
            }
            Err(Error::Busy) => tokio::time::sleep(Duration::from_millis(100)).await,
            Err(error) => return Err(error),
        }
    }
    let owner = owner.ok_or(Error::Busy)?;
    let task = store.show(id)?;
    if task.checks != Checks::Running || listed(&task).is_none() {
        return Ok(task);
    }
    // The suite is the host's own, written and pinned before the run.
    let report = checks::execute(&task, &crate::capability::Trust::everything()).await?;
    owner.record(super::owner::Event::Checked { report })
}

/// Whether `task`'s independent check is still to come or under way: its
/// owner is listing it or running it. A check whose owner is gone is
/// ended unavailable here ([`Store::settle`]), so nothing waits on it.
#[must_use]
pub fn pending(directory: &Path, id: &str) -> bool {
    let Ok(mut store) = Store::open(directory) else {
        return false;
    };
    let Ok(task) = store.show(id) else {
        return false;
    };
    if listed(&task).is_none() {
        return false;
    }
    match task.checks {
        Checks::NotRun => {
            task.status == Status::Finished
                && task.execution == Execution::Finished
                && matches!(Owner::acquire(&store, id), Err(Error::Busy))
        }
        Checks::Running => {
            // Between the run's end and its check the owner lets go of
            // the task for a moment.
            for _ in 0..5 {
                match Owner::acquire(&store, id) {
                    Err(Error::Busy) => return true,
                    Ok(owner) => drop(owner),
                    Err(_) => return false,
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            let _ = store.settle(id);
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
