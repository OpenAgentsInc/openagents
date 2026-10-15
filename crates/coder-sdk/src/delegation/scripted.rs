//! A scripted delegation host (#11511): every harness answers from a
//! script, with no login and no model, so the whole delegation path (the
//! tool on each transport, call-time checks, worktrees, reports, records,
//! nesting and authority) runs in tests.
//!
//! A scripted child makes a real git worktree from the host's repository,
//! writes one file named for its harness, records a passing check, writes
//! its transcript, and reports the way a live child does. With
//! [`ScriptedHost::nesting`] it also delegates once more, to the next
//! harness, through its own [`Delegator`], so nesting runs end to end.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use futures::future::BoxFuture;
use serde_json::{Value, json};

use super::{
    Authority, Check, Child, Delegator, Host, Lineage, Outcome, Report, Request, Target,
    changed_files, closing,
};

/// One scripted harness: its id, name and the models it offers.
#[derive(Clone, Debug)]
pub struct Harness {
    pub id: String,
    pub name: String,
    pub models: Vec<String>,
}

impl Harness {
    #[must_use]
    pub fn new(id: &str, name: &str, models: &[&str]) -> Self {
        Harness {
            id: id.to_owned(),
            name: name.to_owned(),
            models: models.iter().map(|model| (*model).to_owned()).collect(),
        }
    }
}

/// The harnesses Coder delegates between, scripted.
#[must_use]
pub fn every_harness() -> Vec<Harness> {
    vec![
        Harness::new(
            "claude-code",
            "Claude Code",
            &["claude-haiku-5-5", "claude-sonnet-5-5", "claude-opus-5-5"],
        ),
        Harness::new("codex", "Codex", &["gpt-6.1-sol", "gpt-6-astra"]),
        Harness::new("opencode", "OpenCode", &["opencode/big-pickle"]),
        Harness::new("grok-build", "Grok Build", &[]),
        Harness::new("coder", "Coder", &[]),
    ]
}

/// A host whose children run from a script.
#[derive(Clone)]
pub struct ScriptedHost {
    repo: PathBuf,
    harnesses: Vec<Harness>,
    /// Each child delegates once more to the harness after its own.
    nesting: bool,
    /// Children started in the background, by id.
    started: Arc<Mutex<Vec<Child>>>,
    runs: PathBuf,
}

impl ScriptedHost {
    /// A host on the git repository `repo`, offering `harnesses`; child
    /// run folders go under `runs`.
    #[must_use]
    pub fn new(repo: &Path, runs: &Path, harnesses: Vec<Harness>) -> Self {
        ScriptedHost {
            repo: repo.to_owned(),
            harnesses,
            nesting: false,
            started: Arc::default(),
            runs: runs.to_owned(),
        }
    }

    /// Each child delegates once more, to the next harness in the list.
    #[must_use]
    pub fn nesting(mut self) -> Self {
        self.nesting = true;
        self
    }

    /// The children started in the background so far.
    #[must_use]
    pub fn started(&self) -> Vec<Child> {
        self.started
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn next_after(&self, harness: &str) -> Option<String> {
        let at = self.harnesses.iter().position(|h| h.id == harness)?;
        self.harnesses
            .get((at + 1) % self.harnesses.len())
            .map(|h| h.id.clone())
    }
}

impl Host for ScriptedHost {
    fn target(&self, request: &Request) -> Result<Target, String> {
        let ids = || {
            self.harnesses
                .iter()
                .map(|h| h.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let harness = if request.to == "auto" {
            self.harnesses.first()
        } else {
            self.harnesses.iter().find(|h| h.id == request.to)
        }
        .ok_or_else(|| {
            format!(
                "{} is not available here. The agents here are: {}.",
                request.to,
                ids()
            )
        })?;
        if let Some(model) = &request.model
            && !harness.models.contains(model)
        {
            return Err(if harness.models.is_empty() {
                format!("{} takes no model here.", harness.name)
            } else {
                format!(
                    "{} has no model {model}. Its models are: {}.",
                    harness.name,
                    harness.models.join(", ")
                )
            });
        }
        Ok(Target {
            harness: harness.id.clone(),
            name: harness.name.clone(),
            model: request.model.clone(),
        })
    }

    fn run(&self, child: Child) -> BoxFuture<'static, Report> {
        let host = self.clone();
        Box::pin(async move { host.script(child).await })
    }

    fn start(&self, child: Child) -> Result<Report, String> {
        self.started
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(child.clone());
        Ok(Report::for_child(&child, Outcome::Running).summarized("Started in the background."))
    }
}

impl ScriptedHost {
    async fn script(&self, child: Child) -> Report {
        let report = Report::for_child(&child, Outcome::Done);
        let mut transcript = vec![json!({"role": "user", "text": child.briefing})];
        let (cwd, base, branch) = if child.request.read_only {
            (self.repo.clone(), None, None)
        } else {
            let dir = child.run_dir.join("worktree");
            let branch = format!("delegate/{}", child.lineage.id);
            match worktree(&self.repo, &dir, &branch, child.start.as_deref()) {
                Ok(base) => (dir, Some(base), Some(branch)),
                Err(error) => return report.failed(&error),
            }
        };
        if !child.request.read_only {
            let file = format!("{}.txt", child.target.harness);
            let body = format!(
                "{} was here (depth {}, {})\n",
                child.target.name,
                child.lineage.depth,
                child.lineage.path()
            );
            if let Err(error) = std::fs::write(cwd.join(&file), body) {
                return report.failed(&error.to_string());
            }
            transcript.push(json!({"role": "tool", "name": "Write", "input": {"path": file}}));
        }
        // A nested delegation, through the child's own delegator.
        let mut nested = None;
        if self.nesting
            && let Some(next) = self.next_after(&child.target.harness)
        {
            let mut delegator = Delegator::new(
                Arc::new(self.clone()),
                child.lineage.clone(),
                child.authority.clone(),
                &self.runs,
            );
            if let Some(observer) = &child.observer {
                delegator = delegator.with_observer(observer.clone());
            }
            let delegator = Arc::new(delegator);
            let result = delegator
                .delegate(json!({"to": next, "task": "nested step", "read_only": child.request.read_only}))
                .await;
            transcript.push(
                json!({"role": "tool", "name": "delegate", "output": match &result {
                    Ok(report) => report.to_value(),
                    Err(error) => json!({"error": error}),
                }}),
            );
            nested = Some(result);
        }
        let reply = format!(
            "Scripted {} did the task.\n\n```json\n{}\n```",
            child.target.name,
            json!({"summary": format!("{} wrote its file at depth {}.", child.target.name, child.lineage.depth),
            "done": true,
            "checks": [{"command": "true", "passed": true}],
            "nested": nested.as_ref().map(|result| match result {
                Ok(report) => report.child.clone(),
                Err(error) => error.clone(),
            })})
        );
        transcript.push(json!({"role": "assistant", "text": reply}));
        for line in transcript.iter().skip(1) {
            child.observe_line(line);
        }
        write_transcript(&child.transcript(), &transcript);
        let (summary, checks, done) = closing(&reply);
        let mut report = report
            .summarized(&summary)
            .with_checks(checks)
            .with_files(changed_files(&cwd, base.as_deref()));
        if done == Some(false) {
            report.outcome = Outcome::Failed;
        }
        if base.is_some() {
            report.worktree = Some(cwd);
            report.branch = branch;
        }
        report
    }
}

/// A child's lineage and authority as a scripted child process would read
/// them from its environment.
#[must_use]
pub fn from_child_env(child: &Child, root: Authority) -> (Lineage, Authority) {
    let env: Vec<(String, String)> = child.env();
    let get = |name: &str| {
        env.iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    };
    (
        Lineage::from_env(&get, &child.target.harness, "unused"),
        Authority::from_env(&get, root),
    )
}

fn write_transcript(path: &Path, lines: &[Value]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    let _ = std::fs::write(path, text);
}

/// A new worktree of `repo` at `dir` on a new `branch`; returns the base
/// commit.
///
/// # Errors
///
/// Git refused.
pub fn worktree(
    repo: &Path,
    dir: &Path,
    branch: &str,
    start: Option<&str>,
) -> Result<String, String> {
    let git = |args: &[&str]| -> Result<String, String> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
        }
    };
    let base = git(&["rev-parse", start.unwrap_or("HEAD")])?;
    let dir_text = dir.display().to_string();
    git(&["worktree", "add", "-q", "-b", branch, &dir_text, &base])?;
    Ok(base)
}

/// A test repository: `git init` with one commit, in `dir`.
///
/// # Errors
///
/// Git refused.
pub fn init_repo(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test"],
        vec!["config", "commit.gpgsign", "false"],
        // Files read back as written, whatever the machine's Git turns
        // line endings into (Windows `core.autocrlf`, #11695).
        vec!["config", "core.autocrlf", "false"],
    ] {
        run_git(dir, &args)?;
    }
    std::fs::write(dir.join("README.md"), "test\n").map_err(|error| error.to_string())?;
    run_git(dir, &["add", "README.md"])?;
    run_git(dir, &["commit", "-q", "-m", "init"])?;
    Ok(())
}

fn run_git(dir: &Path, args: &[&str]) -> Result<(), String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// A checked [`Check`], for scripted children that ran one.
#[must_use]
pub fn passed(command: &str) -> Check {
    Check {
        command: command.to_owned(),
        passed: true,
    }
}
