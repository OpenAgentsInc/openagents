//! The run's ground: the issue, its base commit, the run's own worktree,
//! and, at the end, the diff, the summary, and the opt-in pull request.
//!
//! Each run owns `STATE/<run-id>/worktree` (#11230, audit RUN-06). Starting
//! a run never touches another run's worktree unless that run finished
//! (its `summary.json` is written), its `change.patch` matches the summary's
//! digest, and `scripts/bench/traces` captured it (`trace-captured.json`
//! names the same digest). A run that is still working, was killed, or was
//! never captured keeps its worktree.

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use serde_json::{Value, json};

use super::{
    Options, Run, Transcript,
    agent::Work,
    decide::Briefing,
    verdict::{self, Outcome, Status},
};
use crate::live::Entry;

/// Where the run works.
#[derive(Clone, Debug)]
pub struct Base {
    pub issue: u64,
    pub title: String,
    pub body: String,
    pub closed: bool,
    /// The commit that fixed a closed issue.
    pub fix: Option<String>,
    /// The commit the run starts from.
    pub base: String,
    /// The repository checkout whose history the run reads.
    pub repo: PathBuf,
    /// The run's own working copy, at `base`.
    pub worktree: PathBuf,
    /// Why `git fetch` failed before an open issue's base was read: the
    /// base may then be a stale `origin/main`.
    pub fetch_error: Option<String>,
}

/// Runs a program and returns its standard output, or a sentence saying
/// why it failed.
pub(crate) async fn output(program: &str, args: &[&str], cwd: &Path) -> Result<String, String> {
    let result = tokio::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|_| format!("Cannot run {program}."))?;
    if result.status.success() {
        Ok(String::from_utf8_lossy(&result.stdout).into_owned())
    } else {
        let error = String::from_utf8_lossy(&result.stderr);
        let line = error.lines().rev().find(|line| !line.trim().is_empty());
        Err(format!(
            "{program} {} failed{}",
            args.first().copied().unwrap_or_default(),
            line.map(|line| format!(": {}", line.trim()))
                .unwrap_or_default()
        ))
    }
}

fn row(label: &str, text: impl Into<String>) -> Value {
    json!({"label": label, "text": text.into()})
}

/// Reads the issue, picks the base commit, and makes the worktree, showing
/// each as a step on one card.
pub async fn prepare(
    options: &Options,
    folder: &Path,
    transcript: &mut Transcript,
) -> Option<Base> {
    let started = Instant::now();
    let mut card = json!({
        "kind": "stage",
        "title": "Issue and starting commit",
        "detail": format!("#{} in {}", options.issue, options.github),
        "rows": [],
    });
    let index = transcript.card(card.clone());
    let fail = |transcript: &mut Transcript, card: &mut Value, why: String| {
        card["ms"] = json!(started.elapsed().as_millis() as u64);
        transcript.finish(index, card.clone(), json!({"error": why}));
        None
    };
    let repo = match options.repo.canonicalize() {
        Ok(repo) => repo,
        Err(_) => {
            return fail(
                transcript,
                &mut card,
                format!("Cannot open the repository {}.", options.repo.display()),
            );
        }
    };
    let number = options.issue.to_string();
    let issue = match output(
        "gh",
        &[
            "issue",
            "view",
            &number,
            "-R",
            &options.github,
            "--json",
            "title,body,state,closedAt,url",
        ],
        &repo,
    )
    .await
    .ok()
    .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    {
        Some(issue) => issue,
        None => {
            return fail(
                transcript,
                &mut card,
                format!("Cannot read issue #{number} with gh. Is gh signed in?"),
            );
        }
    };
    let title = issue["title"].as_str().unwrap_or_default().to_owned();
    let body = issue["body"].as_str().unwrap_or_default().to_owned();
    let closed = issue["state"].as_str() == Some("CLOSED");
    card["detail"] = json!(format!("#{number} {title}"));
    let mut rows = vec![row(
        "state",
        if closed {
            "closed: replay it from the fix's parent and compare with the real fix"
        } else {
            "open: start from the newest main"
        },
    )];
    let mut fetch_error = None;
    let (fix, base) = if closed {
        let fix = match &options.fix {
            Some(fix) => Some(fix.clone()),
            None => find_fix(&repo, options.issue).await,
        };
        let Some(fix) = fix else {
            card["rows"] = json!(rows);
            return fail(
                transcript,
                &mut card,
                format!("No commit on origin/main names #{number}. Pass --fix SHA."),
            );
        };
        let resolved = output("git", &["rev-parse", &format!("{fix}^{{commit}}")], &repo).await;
        let parent = output("git", &["rev-parse", &format!("{fix}^")], &repo).await;
        let (Ok(fix), Ok(parent)) = (resolved, parent) else {
            card["rows"] = json!(rows);
            return fail(
                transcript,
                &mut card,
                format!("The fix commit {fix} is not in this repository."),
            );
        };
        let fix = fix.trim().to_owned();
        let subject = output("git", &["log", "-1", "--format=%s", &fix], &repo)
            .await
            .unwrap_or_default();
        rows.push(row("fix", format!("{} {}", short(&fix), subject.trim())));
        rows.push(row(
            "start",
            format!("{} (the fix's parent)", short(parent.trim())),
        ));
        (Some(fix), parent.trim().to_owned())
    } else {
        if let Err(why) = output("git", &["fetch", "-q", "origin", "main"], &repo).await {
            fetch_error = Some(why);
        }
        let Ok(head) = output("git", &["rev-parse", "origin/main"], &repo).await else {
            card["rows"] = json!(rows);
            return fail(
                transcript,
                &mut card,
                "This repository has no origin/main.".into(),
            );
        };
        rows.push(row(
            "start",
            match &fetch_error {
                None => format!("{} (origin/main)", short(head.trim())),
                Some(why) => format!(
                    "{} (origin/main as last fetched; the fetch failed, so it may be stale: {why})",
                    short(head.trim())
                ),
            },
        ));
        (None, head.trim().to_owned())
    };
    card["rows"] = json!(rows);
    transcript.set(
        index,
        Entry::Tool {
            name: super::DECISION.into(),
            input: card.clone(),
            output: Value::Null,
            running: true,
        },
    );
    let swept = sweep(&options.state, &repo, folder).await;
    let worktree = folder.join("worktree");
    if let Err(why) = make_worktree(&repo, &worktree, &base).await {
        // Only this run's own, just-made, empty worktree is removed.
        let text = worktree.to_string_lossy().into_owned();
        let _ = output("git", &["worktree", "remove", "--force", &text], &repo).await;
        let _ = std::fs::remove_dir_all(&worktree);
        return fail(transcript, &mut card, why);
    }
    rows.push(row(
        "worktree",
        format!("{} (this run's own, sparse, no pushes)", worktree.display()),
    ));
    if swept > 0 {
        rows.push(row(
            "cleaned",
            format!("{swept} finished and captured runs' worktrees"),
        ));
    }
    rows.push(row("folder", folder.display().to_string()));
    card["rows"] = json!(rows);
    card["ms"] = json!(started.elapsed().as_millis() as u64);
    transcript.finish(index, card, json!({"ok": true}));
    Some(Base {
        issue: options.issue,
        title,
        body,
        closed,
        fix,
        base,
        repo,
        worktree,
        fetch_error,
    })
}

pub(crate) fn short(sha: &str) -> &str {
    &sha[..sha.len().min(10)]
}

/// The newest commit on `origin/main` whose message names `#issue`.
async fn find_fix(repo: &Path, issue: u64) -> Option<String> {
    let pattern = format!("#{issue}([^0-9]|$)");
    let log = output(
        "git",
        &[
            "log",
            "origin/main",
            "-E",
            &format!("--grep={pattern}"),
            "--format=%H",
            "-n",
            "1",
        ],
        repo,
    )
    .await
    .ok()?;
    log.lines().next().map(str::to_owned)
}

/// A fresh sparse worktree of `repo` at `base`, at `path`, which must not
/// exist yet: a run never reuses or removes another run's working copy.
async fn make_worktree(repo: &Path, path: &Path, base: &str) -> Result<(), String> {
    let text = path.to_string_lossy().into_owned();
    if path.exists() {
        return Err(format!(
            "{} already exists; a run never reuses another run's worktree.",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| format!("Cannot create {}.", parent.display()))?;
    }
    output(
        "git",
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            "--no-checkout",
            &text,
            base,
        ],
        repo,
    )
    .await?;
    output(
        "git",
        &["sparse-checkout", "set", "--no-cone", "/*", "!/assets/"],
        path,
    )
    .await?;
    output("git", &["checkout", "-q", "--detach", base], path).await?;
    Ok(())
}

/// The marker `scripts/bench/traces` writes into a run folder once its
/// trace is stored.
pub const CAPTURED: &str = "trace-captured.json";

/// The sha256 of `bytes`, as `sha256:HEX`.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    format!("sha256:{:x}", sha2::Sha256::digest(bytes))
}

/// Writes `bytes` to `path` through a temporary file and a rename, so a
/// reader never sees half a file.
///
/// # Errors
/// The file cannot be written or renamed.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes)
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|error| {
            let _ = std::fs::remove_file(&tmp);
            format!("Cannot write {}: {error}", path.display())
        })
}

/// Whether run folder `folder`'s worktree may be deleted: the run finished
/// (its summary is written), its `change.patch` is the diff the summary
/// names, and its trace was captured from that same diff. `Err` says why
/// it must be kept.
///
/// # Errors
/// The worktree holds work that is not both finished and captured.
pub fn cleanable(folder: &Path) -> Result<(), String> {
    let read = |name: &str| -> Option<Value> {
        serde_json::from_slice(&std::fs::read(folder.join(name)).ok()?).ok()
    };
    let Some(summary) = read("summary.json") else {
        return Err("the run has not finished (no summary.json)".into());
    };
    if summary["finished"].as_bool() != Some(true) {
        return Err("the summary does not mark the run finished".into());
    }
    let Some(named) = summary["diff_sha256"].as_str() else {
        return Err("the summary names no diff".into());
    };
    let Ok(patch) = std::fs::read(folder.join("change.patch")) else {
        return Err("change.patch is missing".into());
    };
    if digest(&patch) != named {
        return Err("change.patch is not the diff the summary names".into());
    }
    let Some(captured) = read(CAPTURED) else {
        return Err("the trace has not been captured".into());
    };
    if captured["diff_digest"].as_str() != Some(named) {
        return Err("the captured trace is not this diff".into());
    }
    Ok(())
}

/// Removes the worktrees of other runs under `state` that are finished and
/// captured ([`cleanable`]); every other run's worktree stays. Returns how
/// many were removed.
pub async fn sweep(state: &Path, repo: &Path, current: &Path) -> usize {
    let mut removed = 0;
    let Ok(entries) = std::fs::read_dir(state) else {
        return 0;
    };
    for entry in entries.flatten() {
        let folder = entry.path();
        let worktree = folder.join("worktree");
        if folder == current || !worktree.is_dir() || cleanable(&folder).is_err() {
            continue;
        }
        let owner = std::fs::read(folder.join("summary.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .and_then(|summary| summary["repo"].as_str().map(PathBuf::from))
            .filter(|path| path.is_dir())
            .unwrap_or_else(|| repo.to_owned());
        let text = worktree.to_string_lossy().into_owned();
        let _ = output("git", &["worktree", "remove", "--force", &text], &owner).await;
        let _ = std::fs::remove_dir_all(&worktree);
        if !worktree.exists() {
            removed += 1;
        }
    }
    let _ = output("git", &["worktree", "prune"], repo).await;
    removed
}

/// Everything changed in `worktree` since `base`, new files included,
/// without touching the worktree's own index.
#[must_use]
pub fn working_diff(worktree: &Path, base: &str) -> String {
    let git = |args: &[&str], index: Option<&Path>| {
        let mut command = std::process::Command::new("git");
        command.arg("-C").arg(worktree).args(args);
        if let Some(index) = index {
            command.env("GIT_INDEX_FILE", index);
        }
        command
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let Some(index) = git(
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
        None,
    ) else {
        return String::new();
    };
    let scratch = PathBuf::from(format!("{}.issue-run", index.trim()));
    if std::fs::copy(index.trim(), &scratch).is_err() {
        return String::new();
    }
    let _ = git(&["add", "-A"], Some(&scratch));
    let diff = git(&["diff", "--cached", base], Some(&scratch)).unwrap_or_default();
    let _ = std::fs::remove_file(&scratch);
    diff
}

/// The files a unified diff touches.
#[must_use]
pub fn diff_files(diff: &str) -> Vec<String> {
    let mut files: Vec<String> = diff
        .lines()
        .filter_map(|line| line.strip_prefix("diff --git a/"))
        .filter_map(|rest| rest.split_once(" b/").map(|(_, path)| path.to_owned()))
        .collect();
    files.sort();
    files.dedup();
    files
}

/// What a run's summary is made from. Every stage after setup is
/// optional, so a run that stopped early still leaves a summary.
pub struct Report<'a> {
    pub options: &'a Options,
    pub run: &'a Run,
    pub base: Option<&'a Base>,
    pub briefing: Option<&'a Briefing>,
    pub work: Option<&'a Work>,
    /// The harness's verdicts on the required final checks.
    pub checks: &'a [(String, bool)],
    pub outcome: &'a Outcome,
    pub started: Instant,
    pub folder: &'a Path,
}

/// The ids of the checks the harness reruns at the end and that decide the
/// outcome: the briefing's `check:` entries.
#[must_use]
pub fn required_checks(briefing: &Briefing) -> Vec<String> {
    briefing
        .checks
        .iter()
        .filter(|check| check.id.starts_with("check:"))
        .map(|check| check.id.clone())
        .collect()
}

/// The summary card's numbers. It also writes `change.patch`, the exact
/// diff the summary describes, when the run has a worktree.
#[must_use]
pub fn summary(report: &Report<'_>) -> Value {
    let Report {
        options,
        run,
        base,
        briefing,
        work,
        checks,
        outcome,
        started,
        folder,
    } = *report;
    let diff = base.map(|base| working_diff(&base.worktree, &base.base));
    let changed = diff.as_deref().map(diff_files).unwrap_or_default();
    // The exact diff the summary describes, kept beside it so a trace
    // (#11218, `scripts/bench/traces`) labels from it and replays it.
    let patch_error = diff
        .as_ref()
        .and_then(|diff| write_atomic(&folder.join("change.patch"), diff.as_bytes()).err());
    let diff_sha256 = diff.as_ref().map(|diff| digest(diff.as_bytes()));
    let required = briefing.map(required_checks).unwrap_or_default();
    let check_commands: Vec<Value> = briefing
        .map(|briefing| {
            briefing
                .checks
                .iter()
                .filter(|check| checks.iter().any(|(id, _)| *id == check.id))
                .map(|check| json!({"id": check.id, "argv": check.argv}))
                .collect()
        })
        .unwrap_or_default();
    // Checks the agent may run but the harness does not rerun: reported,
    // never deciding the outcome.
    let optional: Vec<Value> = briefing
        .map(|briefing| {
            briefing
                .checks
                .iter()
                .filter(|check| !required.contains(&check.id))
                .map(|check| {
                    let ok = work.and_then(|work| {
                        work.checks
                            .iter()
                            .find(|(id, _)| *id == check.id)
                            .map(|(_, ok)| *ok)
                    });
                    json!({"id": check.id, "ok": ok, "ran": ok.is_some()})
                })
                .collect()
        })
        .unwrap_or_default();
    let briefed: Vec<&String> = briefing
        .map(|b| b.files.iter().collect())
        .unwrap_or_default();
    let count = |prefix: char, header: &str| {
        diff.as_deref().map(|diff| {
            diff.lines()
                .filter(|line| line.starts_with(prefix) && !line.starts_with(header))
                .count()
        })
    };
    let agent_unknown = match work {
        None => "the agent did not run",
        Some(work) if work.cost_usd.is_none() => "Claude Code reported no cost for the session",
        Some(_) => "",
    };
    let decision_usd = briefing
        .filter(|b| b.decision_unpriced == 0)
        .map(|b| b.decision_usd);
    let decision_unknown = match briefing {
        None => "the decision steps stopped before reporting their cost".to_owned(),
        Some(b) => format!("{} decision answers reported no cost", b.decision_unpriced),
    };
    let cost = verdict::cost(
        verdict::component(work.and_then(|w| w.cost_usd), agent_unknown),
        verdict::component(decision_usd, &decision_unknown),
    );
    let usage_unknown = match work {
        None => Some("the agent did not run"),
        Some(work) if work.input_tokens.is_none() => Some("the agent's session reported no usage"),
        Some(_) => None,
    };
    let mut summary = json!({
        "v": "openagents.coder-issue-run-summary.v2",
        "run_id": run.id,
        "attempt": run.attempt,
        "finished": true,
        "outcome": outcome.to_json(),
        "issue": options.issue,
        "title": base.map(|b| b.title.clone()),
        "repo": base.map(|b| b.repo.display().to_string()),
        "wall_ms": started.elapsed().as_millis() as u64,
        "agent_ms": work.map(|w| w.wall_ms),
        "model": options.model,
        "turns": work.and_then(|w| w.turns),
        "input_tokens": work.and_then(|w| w.input_tokens),
        "output_tokens": work.and_then(|w| w.output_tokens),
        "cache_read_tokens": work.and_then(|w| w.cache_read),
        "cache_write_tokens": work.and_then(|w| w.cache_write),
        "usage_unknown_reason": usage_unknown,
        "agent_usd": work.and_then(|w| w.cost_usd),
        "decision_usd": decision_usd,
        "cost": cost,
        "briefed": briefed,
        "opened_outside_briefing": work.map(|w| w.misses.clone()).unwrap_or_default(),
        "required_checks": required,
        "checks": checks.iter().map(|(id, ok)| json!({"id": id, "ok": ok})).collect::<Vec<_>>(),
        "optional_checks": optional,
        "agent_checks": work.map(|w| w.checks.iter().map(|(id, ok)| json!({"id": id, "ok": ok})).collect::<Vec<_>>()).unwrap_or_default(),
        "changed": changed,
        "base": base.map(|b| b.base.clone()),
        "base_fetch_error": base.and_then(|b| b.fetch_error.clone()),
        "diff_sha256": diff_sha256,
        "patch_error": patch_error,
        "check_commands": check_commands,
        "added": count('+', "+++"),
        "removed": count('-', "---"),
        "error": work.and_then(|w| w.error.clone()).or_else(|| {
            matches!(
                outcome.status,
                Status::SetupFailed | Status::DecisionFailed | Status::Cancelled
            )
            .then(|| outcome.reason.clone())
            .flatten()
        }),
        "reply": work.and_then(|w| w.reply.clone()),
        "folder": folder.display().to_string(),
        "worktree": base.map(|b| b.worktree.display().to_string()),
    });
    if let Some(base) = base
        && let Some(fix) = &base.fix
    {
        let fix_files: Vec<String> = std::process::Command::new("git")
            .arg("-C")
            .arg(&base.repo)
            .args(["diff", "--name-only", &base.base, fix])
            .output()
            .ok()
            .map(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .filter(|line| !line.ends_with("Cargo.lock"))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let both: Vec<&String> = fix_files.iter().filter(|f| changed.contains(f)).collect();
        let missed: Vec<&String> = fix_files.iter().filter(|f| !changed.contains(f)).collect();
        let briefed_fix: Vec<&String> = fix_files
            .iter()
            .filter(|f| briefing.is_some_and(|b| b.files.contains(f)))
            .collect();
        summary["fix"] = json!({
            "commit": short(fix),
            "files": fix_files,
            "both": both,
            "missed": missed,
            "briefed": briefed_fix,
        });
    }
    summary
}

/// `--open-pr`: commit the change to `coder/issue-N`, push it, and open a
/// pull request. Each step shows as a Run.
///
/// It refuses unless `outcome` delivers: a failed or missing required
/// check, an agent error, an unchecked or a cancelled run never commits,
/// pushes, or opens a pull request (#11230).
pub async fn open_pr(
    options: &Options,
    base: &Base,
    outcome: &Outcome,
    transcript: &mut Transcript,
) {
    if !outcome.delivers() {
        transcript.notice(format!(
            "No pull request: the run did not pass ({}). {}",
            outcome.status.as_str(),
            outcome.reason.as_deref().unwrap_or_default()
        ));
        return;
    }
    if base.closed {
        transcript.notice("A closed issue's test run never opens a pull request.");
        return;
    }
    let branch = format!("coder/issue-{}", base.issue);
    let message = format!(
        "{} (#{})\n\nMade by coder issue-run from a briefed Claude agent.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
        base.title, base.issue
    );
    let body = format!(
        "Closes #{}.\n\nMade by `coder issue-run {}`: a briefed Claude agent working from the issue's decision steps.\n\n🤖 Generated with [Claude Code](https://claude.com/claude-code)",
        base.issue, base.issue
    );
    let steps: Vec<Vec<String>> = vec![
        vec!["git".into(), "switch".into(), "-c".into(), branch.clone()],
        vec!["git".into(), "add".into(), "-A".into()],
        vec![
            "git".into(),
            "commit".into(),
            "-q".into(),
            "-m".into(),
            message,
        ],
        vec![
            "git".into(),
            "push".into(),
            "-u".into(),
            "origin".into(),
            format!("HEAD:refs/heads/{branch}"),
        ],
        vec![
            "gh".into(),
            "pr".into(),
            "create".into(),
            "-R".into(),
            options.github.clone(),
            "--head".into(),
            branch.clone(),
            "--base".into(),
            "main".into(),
            "--title".into(),
            format!("{} (#{})", base.title, base.issue),
            "--body".into(),
            body,
        ],
    ];
    for step in steps {
        let shown = step
            .iter()
            .take(if step[0] == "gh" {
                3
            } else {
                step.len().min(5)
            })
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        let index = transcript.push(Entry::Tool {
            name: "Run".into(),
            input: json!({"command": shown}),
            output: Value::Null,
            running: true,
        });
        let args: Vec<&str> = step[1..].iter().map(String::as_str).collect();
        let result = output(&step[0], &args, &base.worktree).await;
        let failed = result.is_err();
        transcript.set(
            index,
            Entry::Tool {
                name: "Run".into(),
                input: json!({"command": shown}),
                output: match result {
                    Ok(text) => json!({"output": text.trim()}),
                    Err(why) => json!({"error": why}),
                },
                running: false,
            },
        );
        if failed {
            return;
        }
    }
}
