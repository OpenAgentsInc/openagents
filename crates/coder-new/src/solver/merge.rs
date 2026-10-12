//! The merge pass over the landed-work ledger: every unlanded run with a
//! pull request, oldest run first, is brought up to date with `main` and
//! merged when GitHub reports it mergeable. A run whose pull request
//! conflicts is rebased once in a scratch worktree and rechecked; one
//! still conflicting gets a handoff comment naming what landed ahead of it. Push
//! and merge are effects: when the merge command itself fails after being
//! sent, the outcome is `unknown` and nothing retries it here.

use std::path::Path;

use serde_json::{Value, json};

use super::ledger;
use crate::issue_run::setup::{main_remote, output};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merged {
    pub run_id: String,
    pub issue: u64,
    pub pull_request: String,
    pub outcome: &'static str,
    pub why: String,
}

impl Merged {
    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "run_id": self.run_id,
            "issue": self.issue,
            "pull_request": self.pull_request,
            "outcome": self.outcome,
            "why": self.why,
        })
    }
}

/// The outcome for one pull request from GitHub's view of it, before any
/// effect: `merge` when it is mergeable, `conflicting` when GitHub says
/// so, `wait` while GitHub has not computed mergeability yet, `skip`
/// when the pull request is no longer open.
#[must_use]
pub fn plan(state: &str, mergeable: &str) -> &'static str {
    match (state, mergeable) {
        ("OPEN", "MERGEABLE") => "merge",
        ("OPEN", "CONFLICTING") => "conflicting",
        ("OPEN", _) => "wait",
        _ => "skip",
    }
}

/// The handoff comment a conflicting pull request receives.
#[must_use]
pub fn handoff(landed: &[String], branch: &str) -> String {
    let ahead = if landed.is_empty() {
        "main moved".to_owned()
    } else {
        format!("these landed ahead of it: {}", landed.join(", "))
    };
    format!(
        "This pull request conflicts with `main` because {ahead}. Rebase `{branch}` on current `main`, rerun the checks, and push; the merge pass takes it on its next run."
    )
}

async fn gh(args: &[&str], repo: &Path) -> Result<String, String> {
    output("gh", args, repo).await
}

/// Rebases `branch` onto the remote's current `main` in a scratch worktree
/// and pushes it with a lease. `Ok(true)` when the branch was rebased and
/// pushed, `Ok(false)` when the rebase hit a conflict (nothing pushed),
/// `Err` when the push was sent and did not confirm.
pub async fn rebase(repo: &Path, branch: &str) -> Result<bool, String> {
    let remote = main_remote(repo).await;
    let dir = std::env::temp_dir().join(format!(
        "solver-merge-{}-{}",
        std::process::id(),
        branch.replace('/', "-")
    ));
    let dir_s = dir.to_string_lossy().into_owned();
    output("git", &["fetch", &remote, "main", branch], repo)
        .await
        .map_err(|e| format!("fetch failed: {e}"))?;
    let _ = output("git", &["worktree", "remove", "--force", &dir_s], repo).await;
    output(
        "git",
        &[
            "worktree",
            "add",
            "--detach",
            &dir_s,
            &format!("{remote}/{branch}"),
        ],
        repo,
    )
    .await
    .map_err(|e| format!("worktree failed: {e}"))?;
    let rebased = output("git", &["rebase", &format!("{remote}/main")], &dir)
        .await
        .is_ok();
    let pushed = if rebased {
        output(
            "git",
            &[
                "push",
                "--force-with-lease",
                &remote,
                &format!("HEAD:{branch}"),
            ],
            &dir,
        )
        .await
        .map(|_| true)
        .map_err(|e| {
            format!(
                "push sent, not confirmed: {}",
                e.lines().last().unwrap_or_default().trim()
            )
        })
    } else {
        let _ = output("git", &["rebase", "--abort"], &dir).await;
        Ok(false)
    };
    let _ = output("git", &["worktree", "remove", "--force", &dir_s], repo).await;
    pushed
}

pub async fn run(state: &Path, repo: &Path, github: &str, dry_run: bool) -> Vec<Merged> {
    let mut results = Vec::new();
    let mut landed: Vec<String> = Vec::new();
    for row in ledger::read(state, repo).await {
        if row.merged.is_some() {
            continue;
        }
        let Some(url) = row.pull_request.clone() else {
            continue;
        };
        let mut result = Merged {
            run_id: row.run_id.clone(),
            issue: row.issue,
            pull_request: url.clone(),
            outcome: "skip",
            why: String::new(),
        };
        let view = gh(
            &[
                "pr",
                "view",
                &url,
                "-R",
                github,
                "--json",
                "state,mergeable,mergeStateStatus,headRefName,mergeCommit",
            ],
            repo,
        )
        .await
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        let Some(view) = view else {
            result.why = "the pull request could not be read".into();
            results.push(result);
            continue;
        };
        let branch = view["headRefName"].as_str().unwrap_or_default().to_owned();
        let state_ = view["state"].as_str().unwrap_or_default();
        let mergeable = view["mergeable"].as_str().unwrap_or_default();
        match plan(state_, mergeable) {
            "merge" => {
                if dry_run {
                    result.outcome = "would_merge";
                } else {
                    if view["mergeStateStatus"] == "BEHIND" {
                        let _ = gh(&["pr", "update-branch", &url, "-R", github], repo).await;
                    }
                    match gh(&["pr", "merge", &url, "-R", github, "--squash"], repo).await {
                        Ok(_) => {
                            result.outcome = "merged";
                            landed.push(format!("#{} ({url})", row.issue));
                        }
                        Err(why) => {
                            result.outcome = "unknown";
                            result.why = format!(
                                "the merge command failed after being sent; check the pull request before retrying: {}",
                                why.lines().last().unwrap_or_default().trim()
                            );
                        }
                    }
                }
            }
            "conflicting" => {
                result.outcome = "conflicting";
                result.why = handoff(&landed, &branch);
                if dry_run {
                    results.push(result);
                    continue;
                }
                match rebase(repo, &branch).await {
                    Ok(true) => {
                        let again = gh(
                            &[
                                "pr",
                                "view",
                                &url,
                                "-R",
                                github,
                                "--json",
                                "mergeable",
                                "-q",
                                ".mergeable",
                            ],
                            repo,
                        )
                        .await
                        .unwrap_or_default();
                        if again.trim() == "MERGEABLE" {
                            match gh(&["pr", "merge", &url, "-R", github, "--squash"], repo).await {
                                Ok(_) => {
                                    result.outcome = "merged";
                                    result.why = "rebased onto main, then merged".into();
                                    landed.push(format!("#{} ({url})", row.issue));
                                }
                                Err(why) => {
                                    result.outcome = "unknown";
                                    result.why = format!(
                                        "the merge command failed after being sent; check the pull request before retrying: {}",
                                        why.lines().last().unwrap_or_default().trim()
                                    );
                                }
                            }
                            results.push(result);
                            continue;
                        }
                        result.why = format!(
                            "rebased and pushed, but GitHub still says {}; {}",
                            again.trim(),
                            result.why
                        );
                    }
                    Ok(false) => {}
                    Err(why) => {
                        result.outcome = "unknown";
                        result.why = why;
                        results.push(result);
                        continue;
                    }
                }
                {
                    let _ = gh(
                        &["pr", "comment", &url, "-R", github, "--body", &result.why],
                        repo,
                    )
                    .await;
                }
            }
            "wait" => {
                result.outcome = "wait";
                result.why = format!("GitHub has not settled mergeability yet ({mergeable})");
            }
            _ => {
                result.why = format!("the pull request is {}", state_.to_lowercase());
            }
        }
        results.push(result);
    }
    results
}

pub async fn command(args: &[String]) -> Result<String, String> {
    let mut state = super::default_state();
    let mut repo = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut github = None;
    let mut dry_run = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--state" => state = args.next().ok_or("--state needs a directory")?.into(),
            "--repo" => repo = args.next().ok_or("--repo needs a path")?.into(),
            "--github" => github = Some(args.next().ok_or("--github needs owner/repo")?.clone()),
            "--dry-run" => dry_run = true,
            other => {
                return Err(format!(
                    "unknown option {other}; coder solver merge takes --github OWNER/REPO, --state DIR, --repo PATH, and --dry-run"
                ));
            }
        }
    }
    let github = match github {
        Some(github) => github,
        None => gh(
            &[
                "repo",
                "view",
                "--json",
                "nameWithOwner",
                "-q",
                ".nameWithOwner",
            ],
            &repo,
        )
        .await?
        .trim()
        .to_owned(),
    };
    let mut out = String::new();
    for result in run(&state, &repo, &github, dry_run).await {
        out.push_str(&result.json().to_string());
        out.push('\n');
    }
    Ok(out)
}
