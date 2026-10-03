//! Publishing a reviewed change of a local run, once.
//!
//! A person reviews a run's change at exact revisions ([`super::review`])
//! and asks to publish it. The publication commits exactly the reviewed
//! tree on the worktree's `HEAD` and pushes that commit as the
//! repository's issue-flow policy says ([`super::issue_run::Policy`],
//! `.openagents/coder-issues.json`): onto the repository's branch,
//! fast-forward only, when the policy lands on `main`; otherwise, and by
//! default, to a branch of its own with a draft pull request. It never
//! rebases, force-pushes, or changes the worktree, its index, or its
//! branch.
//!
//! The operation's identity derives from the task and the reviewed
//! revisions ([`operation_id`]), and its progress is kept beside the task
//! (`<store>/local/<task>.publish.json`) before each effect:
//!
//! - a head the worktree has moved past is refused, and nothing is pushed;
//! - the commit is made once and recorded before the push;
//! - a push whose result is unknown (it timed out, or Git reported no
//!   status for the ref) is recorded as uncertain; publishing again reads
//!   the remote first and pushes only when the commit is not there;
//! - a pull request is looked up by its branch before one is opened;
//! - a publication that finished answers again with its record and
//!   changes nothing.
//!
//! Git hooks run as Git runs them: the commit is made with `git
//! commit-tree`, which names the reviewed tree exactly, and the push runs
//! the repository's pre-push hook. GitHub is reached through the `gh` CLI
//! the person is signed in to ([`GhForge`]); nothing here reads, stores,
//! or prints a token.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use coder_host::access::review::{Landing, Publication, PublishState};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::issue_run::{Land, Policy};
use super::local;
use super::review;

/// The version of the publication file.
pub const PUBLISH_SCHEMA: &str = "openagents.coder.publish.v1";
/// How long a push may take before its result counts as unknown.
pub const PUSH_TIMEOUT: Duration = Duration::from_secs(120);
/// The most publications kept per task.
const KEPT: usize = 16;

/// One publication at a time on this computer: a retry waits for the
/// attempt it repeats.
static PUBLISHING: Mutex<()> = Mutex::new(());

/// The revisions a person reviewed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reviewed {
    pub base: String,
    pub head_commit: String,
    pub head: String,
}

/// Why a publication could not be attempted at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The task was not started on this computer, or has no worktree.
    NoWorktree,
    /// The task's publication file cannot be read or written.
    Store(String),
}

/// The forge a draft pull request opens on.
pub trait Forge: Send + Sync {
    /// `owner/name` of the repository the checkout at `dir` pushes to.
    ///
    /// # Errors
    /// Why it cannot tell, such as a remote that is not on the forge.
    fn repository(&self, dir: &Path) -> Result<String, String>;
    /// The pull request open from `branch`, if one is.
    ///
    /// # Errors
    /// Why the forge could not be read.
    fn find(&self, dir: &Path, repository: &str, branch: &str) -> Result<Option<String>, String>;
    /// Open a draft pull request from `branch` onto `base`; returns its
    /// link.
    ///
    /// # Errors
    /// Why it was not opened.
    fn open_draft(
        &self,
        dir: &Path,
        repository: &str,
        branch: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String>;
}

/// GitHub through the `gh` CLI.
#[derive(Clone, Copy, Debug, Default)]
pub struct GhForge;

fn gh(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("gh")
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .current_dir(dir)
        .output()
        .map_err(|_| {
            "cannot run gh; install the GitHub CLI and sign in with `gh auth login`".to_owned()
        })?;
    if !output.status.success() {
        return Err(clip(String::from_utf8_lossy(&output.stderr).trim(), 400));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

impl Forge for GhForge {
    fn repository(&self, dir: &Path) -> Result<String, String> {
        let url = local::git_out(dir, &["remote", "get-url", "origin"])?;
        github_repository(url.trim())
            .ok_or_else(|| "the remote `origin` is not a GitHub repository".to_owned())
    }

    fn find(&self, dir: &Path, repository: &str, branch: &str) -> Result<Option<String>, String> {
        let url = gh(
            dir,
            &[
                "pr", "list", "-R", repository, "--head", branch, "--state", "open", "--json",
                "url", "--jq", ".[0].url",
            ],
        )?;
        Ok(Some(url).filter(|url| url.starts_with("https://")))
    }

    fn open_draft(
        &self,
        dir: &Path,
        repository: &str,
        branch: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String> {
        gh(
            dir,
            &[
                "pr", "create", "--draft", "-R", repository, "--head", branch, "--base", base,
                "--title", title, "--body", body,
            ],
        )
        .and_then(|url| {
            url.lines()
                .rev()
                .find(|line| line.starts_with("https://"))
                .map(str::to_owned)
                .ok_or_else(|| "gh named no pull request".to_owned())
        })
    }
}

/// `owner/name` of a GitHub remote URL, from its bounded form
/// (`git@github.com:owner/name.git`, `https://github.com/owner/name`).
#[must_use]
pub fn github_repository(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("https://github.com/"))?;
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    let (owner, name) = rest.split_once('/')?;
    let fine = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    (fine(owner) && fine(name)).then(|| format!("{owner}/{name}"))
}

/// The identity of publishing `task` at `reviewed`.
#[must_use]
pub fn operation_id(task: &str, reviewed: &Reviewed) -> String {
    let mut hash = Sha256::new();
    for part in [
        PUBLISH_SCHEMA,
        task,
        &reviewed.base,
        &reviewed.head_commit,
        &reviewed.head,
    ] {
        hash.update(part.as_bytes());
        hash.update([0]);
    }
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Ledger {
    schema: String,
    task: String,
    /// Oldest first.
    publications: Vec<Publication>,
}

fn ledger_path(store: &Path, task: &str) -> PathBuf {
    store.join("local").join(format!("{task}.publish.json"))
}

fn load(store: &Path, task: &str) -> Ledger {
    std::fs::read(ledger_path(store, task))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Ledger>(&bytes).ok())
        .filter(|ledger| ledger.schema == PUBLISH_SCHEMA && ledger.task == task)
        .unwrap_or_else(|| Ledger {
            schema: PUBLISH_SCHEMA.into(),
            task: task.into(),
            publications: Vec::new(),
        })
}

fn keep(store: &Path, ledger: &mut Ledger, publication: &Publication) -> Result<(), Refusal> {
    ledger
        .publications
        .retain(|kept| kept.operation != publication.operation);
    ledger.publications.push(publication.clone());
    while ledger.publications.len() > KEPT {
        ledger.publications.remove(0);
    }
    let bytes = serde_json::to_vec_pretty(ledger).map_err(|e| Refusal::Store(e.to_string()))?;
    super::autostart::write_private(&ledger_path(store, &ledger.task), &bytes)
        .map_err(Refusal::Store)
}

/// The last publication of `task` that reached the remote or may have,
/// if any.
#[must_use]
pub fn last(store: &Path, task: &str) -> Option<Publication> {
    load(store, task).publications.pop()
}

/// The publication of `task` at `reviewed`, when one was attempted.
#[must_use]
pub fn find(store: &Path, task: &str, reviewed: &Reviewed) -> Option<Publication> {
    let operation = operation_id(task, reviewed);
    load(store, task)
        .publications
        .into_iter()
        .find(|kept| kept.operation == operation)
}

/// What a push did.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Pushed {
    Done,
    /// The remote refused it; nothing changed there.
    Rejected(String),
    /// Unknown: it may or may not have reached the remote.
    Unknown(String),
}

/// Publishes reviewed changes of the local runs in one task store.
pub struct Publisher<'a> {
    store: PathBuf,
    forge: &'a dyn Forge,
    push_timeout: Duration,
}

impl<'a> Publisher<'a> {
    /// A publisher over the task store `store`.
    #[must_use]
    pub fn new(store: impl Into<PathBuf>, forge: &'a dyn Forge) -> Self {
        Self {
            store: store.into(),
            forge,
            push_timeout: PUSH_TIMEOUT,
        }
    }

    /// Count a push still running after `timeout` as uncertain.
    #[must_use]
    pub fn with_push_timeout(mut self, timeout: Duration) -> Self {
        self.push_timeout = timeout;
        self
    }

    /// Publish `task`'s change at `reviewed`, once. Every outcome the
    /// repository decided, including a refusal and an uncertain push, is a
    /// [`Publication`].
    ///
    /// # Errors
    /// The task has no worktree here, or its record cannot be kept.
    pub fn publish(&self, task: &str, reviewed: &Reviewed) -> Result<Publication, Refusal> {
        let publication = self.publish_once(task, reviewed)?;
        // The pushed commit holds the worktree's whole content: an ended
        // task's worktree then goes (#10291).
        if matches!(
            publication.state,
            PublishState::Published | PublishState::Pushed
        ) {
            let _ = super::retire::retire(&self.store, task);
        }
        Ok(publication)
    }

    fn publish_once(&self, task: &str, reviewed: &Reviewed) -> Result<Publication, Refusal> {
        let _one = PUBLISHING
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        // A worktree removed when its task ended comes back first (#10291).
        let record = super::retire::ensure(&self.store, task)
            .ok()
            .flatten()
            .ok_or(Refusal::NoWorktree)?;
        let worktree = PathBuf::from(&record.worktree);
        if !worktree.is_dir() {
            return Err(Refusal::NoWorktree);
        }
        let mut ledger = load(&self.store, task);
        let operation = operation_id(task, reviewed);
        let kept = ledger
            .publications
            .iter()
            .find(|kept| kept.operation == operation)
            .cloned();
        if let Some(done) = kept
            .as_ref()
            .filter(|kept| kept.state == PublishState::Published)
        {
            return Ok(done.clone());
        }
        let policy = Policy::load(&worktree);
        let landing = match &policy {
            Ok(policy) if policy.land == Land::Main => Landing::Branch,
            _ => Landing::DraftPullRequest,
        };
        let mut publication = kept.unwrap_or_else(|| Publication {
            operation: operation.clone(),
            task: task.to_owned(),
            base: reviewed.base.clone(),
            head_commit: reviewed.head_commit.clone(),
            head: reviewed.head.clone(),
            landing,
            state: PublishState::Refused,
            branch: None,
            commit: None,
            url: None,
            note: String::new(),
        });
        // A commit already made keeps the landing it was made for.
        let landing = if publication.commit.is_some() {
            publication.landing
        } else {
            landing
        };
        let refuse = |mut publication: Publication, note: String| {
            publication.state = PublishState::Refused;
            publication.note = note;
            Ok(publication)
        };
        let policy = match policy {
            Ok(policy) => policy,
            Err(why) => return refuse(publication, format!("Nothing was published: {why}")),
        };
        // The reviewed base must be the run's own.
        let base = local::git_out(
            &worktree,
            &[
                "rev-parse",
                "--verify",
                &format!("{}^{{commit}}", record.base),
            ],
        )
        .unwrap_or_default();
        if base.trim() != reviewed.base {
            return refuse(
                publication,
                "Nothing was published: the reviewed base is not this task's base.".into(),
            );
        }
        // A commit not yet made needs the worktree still at the reviewed
        // head. Once made, the commit is the reviewed tree, whatever the
        // worktree did since.
        if publication.commit.is_none() {
            match review::head(&worktree) {
                Ok(now) if now.commit == reviewed.head_commit && now.tree == reviewed.head => {}
                Ok(_) => {
                    return refuse(
                        publication,
                        "Nothing was published: the change moved since it was reviewed. \
                         Refresh and review it again."
                            .into(),
                    );
                }
                Err(why) => return refuse(publication, format!("Nothing was published: {why}")),
            }
            if reviewed.head
                == local::git_out(
                    &worktree,
                    &[
                        "rev-parse",
                        &format!("{base}^{{tree}}", base = reviewed.base),
                    ],
                )
                .unwrap_or_default()
                .trim()
            {
                return refuse(
                    publication,
                    "Nothing was published: the task changed nothing.".into(),
                );
            }
        }
        let target = policy
            .branch
            .clone()
            .unwrap_or_else(|| local::default_branch(&worktree));
        let branch = publication.branch.clone().unwrap_or_else(|| match landing {
            Landing::Branch => target.clone(),
            Landing::DraftPullRequest => format!("coder/review-{}-{}", &task[..8], &operation[..8]),
        });
        publication.landing = landing;
        publication.branch = Some(branch.clone());
        let title = self.title(task);
        let commit = match publication.commit.clone() {
            Some(commit) => commit,
            None => match self.commit(&worktree, reviewed, &title, task, &policy) {
                Ok(commit) => {
                    publication.commit = Some(commit.clone());
                    // Recorded before the push: a crash during it reads the
                    // remote before pushing again.
                    publication.state = PublishState::Uncertain;
                    publication.note = "Pushing the reviewed change.".into();
                    keep(&self.store, &mut ledger, &publication)?;
                    commit
                }
                Err(why) => return refuse(publication, why),
            },
        };
        let short = &commit[..10];
        // Pushed already? Only a commit that may have reached the remote is
        // looked for; a new one has not.
        let there = if publication.state == PublishState::Uncertain
            || publication.state == PublishState::Pushed
        {
            match on_remote(&worktree, &branch, &commit) {
                Ok(there) => there,
                Err(why) => {
                    publication.state = PublishState::Uncertain;
                    publication.note = format!(
                        "Coder could not read the remote to see whether {short} reached it: {why}"
                    );
                    keep(&self.store, &mut ledger, &publication)?;
                    return Ok(publication);
                }
            }
        } else {
            false
        };
        if !there {
            match push(&worktree, &commit, &branch, self.push_timeout) {
                Pushed::Done => {}
                Pushed::Rejected(why) => {
                    publication.state = PublishState::Refused;
                    publication.note = match landing {
                        Landing::Branch => format!(
                            "The remote refused {short} on {branch} ({why}); {branch} moved \
                             since the base. Nothing was published."
                        ),
                        Landing::DraftPullRequest => {
                            format!("The remote refused {branch} ({why}). Nothing was published.")
                        }
                    };
                    keep(&self.store, &mut ledger, &publication)?;
                    return Ok(publication);
                }
                Pushed::Unknown(why) => {
                    publication.state = PublishState::Uncertain;
                    publication.note = format!(
                        "Not sure the push of {short} reached the remote ({why}). Publishing \
                         again checks the remote first."
                    );
                    keep(&self.store, &mut ledger, &publication)?;
                    return Ok(publication);
                }
            }
        }
        let repository = self.forge.repository(&worktree);
        match landing {
            Landing::Branch => {
                publication.state = PublishState::Published;
                publication.url = repository
                    .as_ref()
                    .ok()
                    .map(|repo| format!("https://github.com/{repo}/commit/{commit}"));
                publication.note = format!("Published {short} on {branch}.");
            }
            Landing::DraftPullRequest => {
                let opened = repository.and_then(|repo| {
                    match self.forge.find(&worktree, &repo, &branch)? {
                        Some(url) => Ok(url),
                        None => self.forge.open_draft(
                            &worktree,
                            &repo,
                            &branch,
                            &target,
                            &title,
                            &format!(
                                "A change Coder made, reviewed in an OpenAgents chat at \
                                 {short}.\n\nTask: `{task}`\nBase: `{}`\nReviewed tree: `{}`",
                                reviewed.base, reviewed.head
                            ),
                        ),
                    }
                });
                match opened {
                    Ok(url) => {
                        publication.state = PublishState::Published;
                        publication.url = Some(url);
                        publication.note =
                            format!("Pushed {short} to {branch} and opened a draft pull request.");
                    }
                    Err(why) => {
                        publication.state = PublishState::Pushed;
                        publication.note = format!(
                            "Pushed {short} to {branch}; the draft pull request was not \
                             opened: {}",
                            clip(&why, 300)
                        );
                    }
                }
            }
        }
        keep(&self.store, &mut ledger, &publication)?;
        Ok(publication)
    }

    fn title(&self, task: &str) -> String {
        super::Store::open(&self.store)
            .ok()
            .and_then(|store| store.show(task).ok())
            .map(|task| clip(task.intent.title.trim(), 72))
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| "A change Coder made".to_owned())
    }

    /// Commit exactly the reviewed tree on the reviewed `HEAD`.
    fn commit(
        &self,
        worktree: &Path,
        reviewed: &Reviewed,
        title: &str,
        task: &str,
        policy: &Policy,
    ) -> Result<String, String> {
        let mut body = format!(
            "Worked by Coder in an OpenAgents chat and published after review.\n\nTask: {task}"
        );
        if let Some(trailer) = &policy.trailer {
            body.push_str(&format!("\n\n{}", trailer.trim()));
        }
        local::git_out(
            worktree,
            &[
                "commit-tree",
                &reviewed.head,
                "-p",
                &reviewed.head_commit,
                "-m",
                title,
                "-m",
                &body,
            ],
        )
        .map(|commit| commit.trim().to_owned())
        .map_err(|why| {
            format!(
                "Git could not commit the reviewed change: {}",
                clip(&why, 300)
            )
        })
    }
}

/// Whether `commit` is on the remote's `branch`: the branch names it or a
/// commit after it.
fn on_remote(worktree: &Path, branch: &str, commit: &str) -> Result<bool, String> {
    let listed = local::git_out(
        worktree,
        &["ls-remote", "origin", &format!("refs/heads/{branch}")],
    )
    .map_err(|why| clip(&why, 300))?;
    let Some(tip) = listed.split_whitespace().next().map(str::to_owned) else {
        return Ok(false);
    };
    if tip == commit {
        return Ok(true);
    }
    local::git_out(
        worktree,
        &["fetch", "-q", "origin", &format!("refs/heads/{branch}")],
    )
    .map_err(|why| clip(&why, 300))?;
    Ok(local::git_out(worktree, &["merge-base", "--is-ancestor", commit, &tip]).is_ok())
}

/// Push `commit` to the remote's `branch`, fast-forward only, within
/// `timeout`. Git's porcelain status line for the ref decides the result;
/// no status line, or no answer in time, is unknown.
fn push(worktree: &Path, commit: &str, branch: &str, timeout: Duration) -> Pushed {
    let destination = format!("refs/heads/{branch}");
    let mut child = match local::git()
        .arg("-C")
        .arg(coder_boundary::plain_path(worktree))
        .args([
            "push",
            "--porcelain",
            "origin",
            &format!("{commit}:{destination}"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return Pushed::Rejected("cannot run git".into()),
    };
    let reader = |stream: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = Vec::new();
            if let Some(mut stream) = stream {
                let _ = stream.read_to_end(&mut text);
            }
            String::from_utf8_lossy(&text).into_owned()
        })
    };
    let out = reader(
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
    );
    let err = reader(
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
    );
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let Some(status) = status else {
        // The readers may wait on a hook's descendants; leave them.
        return Pushed::Unknown(format!("no answer in {} seconds", timeout.as_secs().max(1)));
    };
    let out = out.join().unwrap_or_default();
    let err = err.join().unwrap_or_default();
    // `<flag>\t<from>:<to>\t<summary>`
    let line = out.lines().find(|line| {
        line.split('\t')
            .nth(1)
            .and_then(|refs| refs.split_once(':'))
            .is_some_and(|(_, to)| to == destination)
    });
    match line.and_then(|line| line.chars().next()) {
        Some('!') => Pushed::Rejected(clip(
            line.and_then(|line| line.split('\t').nth(2))
                .unwrap_or("rejected"),
            200,
        )),
        Some(' ' | '+' | '-' | '*' | '=') if status.success() => Pushed::Done,
        _ => Pushed::Unknown(clip(err.trim(), 300)),
    }
}

fn clip(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

#[cfg(test)]
#[path = "publish_tests.rs"]
mod tests;
