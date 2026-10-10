//! The run's ground: the issue, its base commit, the fresh worktree, and,
//! at the end, the diff, the summary, and the opt-in pull request.

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use serde_json::{Value, json};

use super::{Options, Transcript, agent::Work, decide::Briefing};
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
        let _ = output("git", &["fetch", "-q", "origin", "main"], &repo).await;
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
            format!("{} (origin/main)", short(head.trim())),
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
    let worktree = options.state.join("worktree");
    if let Err(why) = make_worktree(&repo, &worktree, &base).await {
        return fail(transcript, &mut card, why);
    }
    rows.push(row(
        "worktree",
        format!("{} (sparse, no pushes)", worktree.display()),
    ));
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

/// A fresh sparse worktree of `repo` at `base`, at a fixed path so the
/// shared build cache keeps its work between runs.
async fn make_worktree(repo: &Path, path: &Path, base: &str) -> Result<(), String> {
    let text = path.to_string_lossy().into_owned();
    if path.exists() {
        let _ = output("git", &["worktree", "remove", "--force", &text], repo).await;
        let _ = std::fs::remove_dir_all(path);
    }
    let _ = output("git", &["worktree", "prune"], repo).await;
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

/// The summary card's numbers.
#[must_use]
pub fn summary(
    options: &Options,
    base: &Base,
    briefing: &Briefing,
    work: &Work,
    checks: &[(String, bool)],
    started: Instant,
    folder: &Path,
) -> Value {
    let diff = working_diff(&base.worktree, &base.base);
    let changed = diff_files(&diff);
    let briefed: Vec<&String> = briefing.files.iter().collect();
    let added = diff
        .lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .count();
    let removed = diff
        .lines()
        .filter(|line| line.starts_with('-') && !line.starts_with("---"))
        .count();
    let mut summary = json!({
        "issue": base.issue,
        "title": base.title,
        "wall_ms": started.elapsed().as_millis() as u64,
        "agent_ms": work.wall_ms,
        "model": options.model,
        "turns": work.turns,
        "input_tokens": work.input_tokens,
        "output_tokens": work.output_tokens,
        "cache_read_tokens": work.cache_read,
        "cache_write_tokens": work.cache_write,
        "agent_usd": work.cost_usd,
        "decision_usd": briefing.decision_usd,
        "briefed": briefed,
        "opened_outside_briefing": work.misses,
        "checks": checks.iter().map(|(id, ok)| json!({"id": id, "ok": ok})).collect::<Vec<_>>(),
        "agent_checks": work.checks.iter().map(|(id, ok)| json!({"id": id, "ok": ok})).collect::<Vec<_>>(),
        "changed": changed,
        "added": added,
        "removed": removed,
        "error": work.error,
        "reply": work.reply,
        "folder": folder.display().to_string(),
        "worktree": base.worktree.display().to_string(),
    });
    if let Some(fix) = &base.fix {
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
            .filter(|f| briefing.files.contains(f))
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
pub async fn open_pr(options: &Options, base: &Base, transcript: &mut Transcript) {
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
