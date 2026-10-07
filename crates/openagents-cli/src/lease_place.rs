//! Placement for `openagents lease run --class CLASS` and `openagents lease
//! RESOURCE --class CLASS` (#10767): decide where a job runs, and run it on
//! another computer over `crates/coder-ssh` when the decision says so.
//! `docs/coder/runtime/placement.md` is the guide.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use coder_lease::placement::{Class, Decision, Place, Policy, Receipt, Target, decide};

/// Decides where a job of `class` runs, probing configured computers over
/// `ssh` only when the placement could go remote.
pub(crate) fn place(class: Class, explicit: Option<&Place>) -> Result<Decision, String> {
    let policy = Policy::from_env()?;
    decide(class, &policy, explicit, &mut |computer| {
        eprintln!("openagents lease: asking whether {computer} answers over SSH");
        coder_ssh::reachable(computer, None)
    })
}

/// The one-line hint a waiting build prints when a computer is configured.
pub(crate) fn build_hint() -> Option<String> {
    let policy = Policy::from_env().ok()?;
    let computer = policy.computers.first()?;
    Some(format!(
        "openagents lease: to build on {computer} instead, run `openagents lease run --class build --place remote -- CMD`"
    ))
}

/// A placement receipt for a job that ran here under `lease`.
pub(crate) fn local_receipt(
    decision: &Decision,
    command: &[String],
    started_at_ms: u64,
    exit: Option<i32>,
    lease: &coder_lease::Receipt,
    fetch: &[String],
) -> Receipt {
    let top = git_top();
    Receipt {
        schema: coder_lease::placement::RECEIPT_SCHEMA.to_owned(),
        class: decision.class,
        asked: decision.asked.clone(),
        place: "local".to_owned(),
        computer: None,
        reason: decision.reason.clone(),
        commit: git(&["rev-parse", "HEAD"]),
        remote_dir: None,
        command: command.to_vec(),
        started_at_ms,
        ended_at_ms: coder_lease::now_ms(),
        exit,
        leases: vec![lease.clone()],
        // Here the results are already in place.
        fetched: fetch
            .iter()
            .map(|path| local_path(top.as_deref(), path))
            .filter(|path| path.exists())
            .map(|path| path.display().to_string())
            .collect(),
    }
}

/// Runs `command` on `computer` at this checkout's pushed commit, streaming
/// its output here, and copies each `fetch` path back to the same place in
/// this checkout. Returns the receipt and, when something failed outside
/// the command, why.
pub(crate) fn run_remote(
    decision: &Decision,
    command: &[String],
    fetch: &[String],
) -> Result<(Receipt, Option<String>), String> {
    let Target::Remote(computer) = &decision.target else {
        return Err("the job was not placed on another computer".to_owned());
    };
    let pushed = pushed_commit()?;
    let job = coder_ssh::Job::new(
        computer,
        &pushed.repository,
        &pushed.commit,
        command.to_vec(),
    )
    .map_err(|error| error.to_string())?;
    eprintln!(
        "openagents lease: running on {computer} at {} in {}: {}",
        &pushed.commit[..12],
        job.remote_dir(),
        decision.reason
    );
    let started_at_ms = coder_lease::now_ms();
    let mut failure = None;
    let exit = match job.run(Stdio::inherit(), Stdio::inherit()) {
        Ok(code) => {
            if code == coder_ssh::SETUP_FAILED {
                failure = Some(format!(
                    "the checkout on {computer} could not be prepared, so the command may not have run"
                ));
            } else if code == 255 {
                failure = Some(format!("ssh to {computer} failed"));
            }
            Some(code)
        }
        Err(error) => {
            failure = Some(format!("the run on {computer} did not finish: {error}"));
            None
        }
    };
    let top = git_top();
    let mut fetched = Vec::new();
    for path in fetch {
        let to = local_path(top.as_deref(), path);
        match job.fetch(path, &to) {
            Ok(()) => fetched.push(to.display().to_string()),
            Err(error) => {
                eprintln!("openagents lease: {path} was not copied back from {computer}: {error}");
            }
        }
    }
    Ok((
        Receipt {
            schema: coder_lease::placement::RECEIPT_SCHEMA.to_owned(),
            class: decision.class,
            asked: decision.asked.clone(),
            place: "remote".to_owned(),
            computer: Some(computer.clone()),
            reason: decision.reason.clone(),
            commit: Some(pushed.commit),
            remote_dir: Some(job.remote_dir()),
            command: command.to_vec(),
            started_at_ms,
            ended_at_ms: coder_lease::now_ms(),
            exit,
            leases: Vec::new(),
            fetched,
        },
        failure,
    ))
}

/// Writes `receipt` under the lease root's `placements/` and, when given,
/// to `copy`.
pub(crate) fn record(root: &Path, receipt: &Receipt, copy: Option<&Path>) -> Result<(), String> {
    let name = format!(
        "{}-{}-{}.json",
        receipt.class,
        receipt.started_at_ms,
        std::process::id()
    );
    receipt
        .write(&root.join("placements").join(name))
        .map_err(|error| format!("the placement receipt was not written: {error}"))?;
    if let Some(copy) = copy {
        receipt
            .write(copy)
            .map_err(|error| format!("{} was not written: {error}", copy.display()))?;
    }
    Ok(())
}

struct Pushed {
    repository: String,
    commit: String,
}

/// This checkout's commit, once it's clean and pushed, with the URL of
/// `origin`, which the other computer clones.
fn pushed_commit() -> Result<Pushed, String> {
    let commit = git(&["rev-parse", "HEAD"])
        .ok_or("a remote run needs a Git checkout; run it here with --place local")?;
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).unwrap_or_default();
    if !dirty.is_empty() {
        return Err(
            "this checkout has uncommitted changes, and the other computer runs the pushed commit; \
             commit and push them, or run it here with --place local"
                .to_owned(),
        );
    }
    let remotes = git(&["branch", "--remotes", "--contains", &commit]).unwrap_or_default();
    if remotes.is_empty() {
        return Err(format!(
            "commit {} is not pushed, and the other computer can only fetch a pushed commit; \
             push it, or run it here with --place local",
            &commit[..commit.len().min(12)]
        ));
    }
    let repository = git(&["remote", "get-url", "origin"])
        .ok_or("this checkout has no `origin` remote for the other computer to clone")?;
    Ok(Pushed { repository, commit })
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn git_top() -> Option<PathBuf> {
    git(&["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

/// Where a result path, relative to the checkout's top, lands here.
fn local_path(top: Option<&Path>, path: &str) -> PathBuf {
    top.map_or_else(|| PathBuf::from(path), |top| top.join(path))
}
