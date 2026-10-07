//! A studio task's Git: its own worktree and branch, the run record a
//! review reads, the local merge a person approves, and the environment
//! that keeps its processes from pushing (`docs/verse/agent-studio.md`;
//! the design follows AgentCraft's Foreman, reimplemented here).
//!
//! - **Worktrees.** On release, the coordinator gives each task a
//!   worktree of its own under the host's state ([`worktrees_dir`]), on a
//!   new branch `studio/<seat>/<task>-<slug>` from the commit the person's
//!   checkout has checked out ([`prepare`]). The task's workspace is that
//!   worktree, never the person's checkout. The coordinator saves the
//!   run record ([`super::super::local::Record`]) beside the task, so a
//!   review reads the task's change at exact revisions even when the
//!   owner's auto-start policy, not a local run, started it.
//! - **Merge.** A person's approved merge ([`merge`]) is built off-tree:
//!   `git merge-tree --write-tree` joins the checkout's branch with the
//!   task's reviewed commit, `git commit-tree` makes the merge commit as
//!   the person (their own Git identity, and signed when their Git
//!   configuration signs commits), and `git merge --ff-only` moves the
//!   checked-out branch to it. A checkout with uncommitted changes, a
//!   detached checkout, and a merge that would conflict are refused with
//!   the reason, which reopens the decision; the checkout is untouched.
//!   A conflict is also noted for the coordinator, which sends the task
//!   back to its worker to merge the branch in ([`super::flow`]), and a
//!   change the lead still reviews is refused until the review ends.
//!   Nothing is pushed and no pull request is opened.
//! - **Push block.** A studio task's processes run with [`confine`]'s
//!   variables: no Git transport at all (`GIT_ALLOW_PROTOCOL` and
//!   `protocol.allow=never`), every push URL rewritten to one Git cannot
//!   use, no signing, repository discovery that stops at the worktree's
//!   parent (`GIT_CEILING_DIRECTORIES`), no inherited variable that points
//!   Git at another repository, and the seat's own identity
//!   (`Studio <Seat>`). Environment-scoped configuration outranks the
//!   repository's and the user's, so neither turns pushing back on. It is
//!   not a sandbox: a process that unsets the variables on purpose can
//!   still reach a remote the filesystem boundary lets it reach.
//! - **Seat signatures.** A host may register a [`SeatSigner`] for its
//!   task store ([`set_seat_signer`]); a merge then hands it the tip
//!   commit of the seat's change, and a workshop agent who signs her
//!   commits (`super::super::agent_git_sign`) returns it signed with her
//!   key under NIP-GS. A signer that fails refuses the merge.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use coder_host::access::review::{Landing, MAX_NOTE, Publication, PublishState};

use super::super::local::{self, RECORD_SCHEMA, Record};
use super::super::publish::{self, Refusal, Reviewed};
use super::super::{retire, review};
use super::{DIR, STATE_FILE, State};

/// The branch namespace of studio tasks.
pub const BRANCH_PREFIX: &str = "studio";
/// The protocol list Git is allowed: a name no transport has.
pub const NO_PROTOCOL: &str = "openagents-studio-none";
/// Where every push URL is rewritten: a URL Git cannot use.
pub const PUSH_BLOCKED: &str = "openagents-studio-push-blocked:///";
/// A signing program that does not exist, so a forced signature fails.
pub const NO_SIGNING: &str = "openagents-studio-signing-disabled";
/// Variables that point Git at another repository, work tree, or index.
/// A studio task's processes never inherit them.
pub const REDIRECTS: [&str; 12] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_CONFIG",
    "GIT_REPLACE_REF_BASE",
    "GIT_SHALLOW_FILE",
    "GIT_GRAFT_FILE",
];
/// The identity variables a seat's commits carry.
const IDENTITY: [&str; 4] = [
    "GIT_AUTHOR_NAME",
    "GIT_AUTHOR_EMAIL",
    "GIT_COMMITTER_NAME",
    "GIT_COMMITTER_EMAIL",
];
const CEILING: &str = "GIT_CEILING_DIRECTORIES";
const COUNT: &str = "GIT_CONFIG_COUNT";
/// The most bytes of a branch name's slug.
const SLUG_MAX: usize = 24;
/// The most bytes of Git's error output a refusal keeps.
const REASON_MAX: usize = 400;

/// One merge at a time on this computer: a retry waits for the attempt
/// it repeats, and two merges never race on one checkout.
static MERGING: Mutex<()> = Mutex::new(());

/// Where the studio's task worktrees live under the host root `root`.
#[must_use]
pub fn worktrees_dir(root: &Path) -> PathBuf {
    root.join("studio-worktrees")
}

/// The seat whose task `task` is, read from the studio document of the
/// task store at `store` without taking its lock (the document is only
/// ever replaced whole). `None` for a task the studio does not hold.
#[must_use]
pub fn seat_of(store: &Path, task: &str) -> Option<String> {
    let bytes = std::fs::read(store.join(DIR).join(STATE_FILE)).ok()?;
    let state: State = serde_json::from_slice(&bytes).ok()?;
    state.goals.iter().find_map(|goal| {
        std::iter::once(&goal.lead)
            .chain(goal.plan.iter().map(|entry| &entry.slot))
            .find(|slot| slot.task_id == task)
            .map(|slot| slot.seat.clone())
    })
}

/// The name and email a seat's commits carry: `Studio Ada`,
/// `ada@studio.invalid`.
#[must_use]
pub fn identity(seat: &str) -> (String, String) {
    let mut chars = seat.chars();
    let name = match chars.next() {
        Some(first) => format!("Studio {}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => "Studio".to_owned(),
    };
    (name, format!("{seat}@studio.invalid"))
}

/// A branch-safe slug of `title`: lowercase letters and digits joined by
/// single hyphens, at most [`SLUG_MAX`] bytes, or `task` when nothing is
/// left.
#[must_use]
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for ch in title.chars() {
        if out.len() >= SLUG_MAX {
            break;
        }
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "task".to_owned()
    } else {
        out.to_owned()
    }
}

/// The branch of seat `seat`'s task `task` titled `title`:
/// `studio/<seat>/<first 8 of task>-<slug>`.
#[must_use]
pub fn branch(seat: &str, task: &str, title: &str) -> String {
    let short = &task[..task.len().min(8)];
    format!("{BRANCH_PREFIX}/{seat}/{short}-{}", slug(title))
}

/// The variables a studio task's process adds over an environment that
/// already holds `variables` but no Git configuration of ours:
/// [`confine`] without the configuration entries, for a caller that can
/// only add variables. `GIT_ALLOW_PROTOCOL` alone already refuses every
/// transport.
#[must_use]
pub fn additions(seat: &str, worktree: &Path) -> Vec<(OsString, OsString)> {
    let (name, email) = identity(seat);
    let mut out: Vec<(OsString, OsString)> = vec![
        ("GIT_ALLOW_PROTOCOL".into(), NO_PROTOCOL.into()),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GCM_INTERACTIVE".into(), "never".into()),
        ("GIT_AUTHOR_NAME".into(), name.clone().into()),
        ("GIT_AUTHOR_EMAIL".into(), email.clone().into()),
        ("GIT_COMMITTER_NAME".into(), name.into()),
        ("GIT_COMMITTER_EMAIL".into(), email.into()),
    ];
    if let Some(parent) = worktree.parent() {
        out.push((CEILING.into(), parent.into()));
    }
    out
}

/// Confine `variables`, a studio task's whole process environment, to
/// local Git work in `worktree` as seat `seat`: drop the redirecting
/// variables, append the push-blocking configuration after any
/// environment-scoped entries already there, and add [`additions`],
/// keeping an existing ceiling after the worktree's parent.
pub fn confine(variables: &mut Vec<(OsString, OsString)>, seat: &str, worktree: &Path) {
    let already = variables
        .iter()
        .find(|(key, _)| key == COUNT)
        .and_then(|(_, value)| value.to_str()?.parse::<usize>().ok())
        .unwrap_or(0);
    let ceiling = variables
        .iter()
        .find(|(key, _)| key == CEILING)
        .map(|(_, value)| value.clone())
        .filter(|value| !value.is_empty());
    let added = additions(seat, worktree);
    variables.retain(|(key, _)| {
        let key = key.to_string_lossy();
        let upper = key.to_ascii_uppercase();
        !REDIRECTS.contains(&upper.as_str())
            && key != COUNT
            && !added.iter().any(|(name, _)| name.to_string_lossy() == key)
    });
    let pairs = [
        ("protocol.allow".to_owned(), String::from("never")),
        (format!("url.{PUSH_BLOCKED}.pushInsteadOf"), String::new()),
        ("commit.gpgsign".to_owned(), "false".to_owned()),
        ("tag.gpgsign".to_owned(), "false".to_owned()),
        ("gpg.program".to_owned(), NO_SIGNING.to_owned()),
        ("gpg.ssh.program".to_owned(), NO_SIGNING.to_owned()),
        ("gpg.x509.program".to_owned(), NO_SIGNING.to_owned()),
    ];
    variables.push((COUNT.into(), (already + pairs.len()).to_string().into()));
    for (offset, (key, value)) in pairs.into_iter().enumerate() {
        let n = already + offset;
        variables.push((format!("GIT_CONFIG_KEY_{n}").into(), key.into()));
        variables.push((format!("GIT_CONFIG_VALUE_{n}").into(), value.into()));
    }
    for (key, value) in added {
        let value = match (&ceiling, key == CEILING) {
            (Some(existing), true) => {
                let mut joined = value;
                joined.push(":");
                joined.push(existing);
                joined
            }
            _ => value,
        };
        variables.push((key, value));
    }
}

/// Git in `dir` with no inherited variable that points it elsewhere, and
/// `with` added.
fn run(dir: &Path, args: &[&str], with: &[(&str, &str)]) -> Result<std::process::Output, String> {
    let mut command = local::git();
    command.arg("-C").arg(coder_boundary::plain_path(dir));
    for name in REDIRECTS.iter().chain(IDENTITY.iter()) {
        command.env_remove(name);
    }
    command
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(with.iter().copied())
        .args(args)
        .output()
        .map_err(|_| "cannot run git".to_owned())
}

/// [`run`]'s trimmed standard output, or its error output as a reason.
fn out(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = run(dir, args, &[])?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(reason(&output))
    }
}

fn reason(output: &std::process::Output) -> String {
    let text = String::from_utf8_lossy(if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    })
    .trim()
    .to_owned();
    clip(&text, REASON_MAX)
}

fn clip(text: &str, max: usize) -> String {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// Give seat `seat`'s task `task`, titled `title`, its own worktree under
/// `worktrees` on a new branch from the commit the checkout at
/// `repository` has checked out, and save its run record in the task
/// store at `store`. A task that already has a record keeps its worktree,
/// so a repeated release changes nothing. Returns the worktree's path.
///
/// # Errors
/// A plain sentence when the repository is not a Git checkout with a
/// commit, or the worktree or the record cannot be made.
pub fn prepare(
    worktrees: &Path,
    store: &Path,
    repository: &Path,
    seat: &str,
    task: &str,
    title: &str,
    requested: Option<&str>,
) -> Result<PathBuf, String> {
    if let Some(record) = local::record(store, task) {
        return Ok(PathBuf::from(record.worktree));
    }
    let top = PathBuf::from(
        out(repository, &["rev-parse", "--show-toplevel"])
            .map_err(|why| format!("{} is not a Git checkout: {why}", repository.display()))?,
    );
    crate::private::create_dir_all(worktrees)
        .map_err(|error| format!("cannot create {}: {error}", worktrees.display()))?;
    let path = worktrees.join(task);
    let at = path.to_string_lossy().into_owned();
    let base = if path.join(".git").exists() {
        // A release a crash interrupted made it; nothing ran in it yet.
        out(&path, &["rev-parse", "--verify", "HEAD^{commit}"])?
    } else {
        let base = out(&top, &["rev-parse", "--verify", "HEAD^{commit}"])
            .map_err(|why| format!("the checkout has no commit to start from: {why}"))?;
        let name = branch(seat, task, title);
        // `-B`: a branch an interrupted release left is this task's own.
        out(&top, &["worktree", "add", "-q", "-B", &name, &at, &base])
            .map_err(|why| format!("Git cannot make the task's worktree: {why}"))?;
        base
    };
    let record = Record {
        schema: RECORD_SCHEMA.into(),
        task: task.into(),
        thread: None,
        project: top
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
        checkout: top.to_string_lossy().into_owned(),
        worktree: at,
        base,
        turns: Vec::new(),
        ends: std::collections::BTreeMap::new(),
        requested: requested.map(str::to_owned),
        shape: local::Shape::default(),
        hooks: None,
        archived: None,
    };
    local::save(store, &record)?;
    Ok(path)
}

/// Merge studio task `task`'s change at the `reviewed` revisions into the
/// branch its checkout has checked out, once; see the module docs. Every
/// outcome the checkout decided, a refusal included, is a [`Publication`]
/// kept beside the task, so the review shows it. A merge that finished
/// answers again with its record and changes nothing.
///
/// # Errors
/// The task has no worktree here, or its record cannot be kept.
/// Merge `target` into task `task`'s worktree as the host, for a seat sent
/// back to resolve a conflict: the seat's run cannot write the common Git
/// directory, so the host commits the seat's uncommitted work on its
/// branch and starts the merge, which leaves Git's conflict markers in the
/// working tree. Returns the conflicting files; the seat resolves them by
/// editing, and the merge's commit keeps `target` as its second parent.
pub fn resolve_conflict(store: &Path, task: &str, target: &str) -> Result<Vec<String>, String> {
    let record = retire::ensure(store, task)
        .ok()
        .flatten()
        .ok_or("the task has no worktree")?;
    let worktree = PathBuf::from(&record.worktree);
    let seat = seat_of(store, task).unwrap_or_else(|| "worker".into());
    let (name, email) = identity(&seat);
    let who = [
        ("GIT_AUTHOR_NAME", name.as_str()),
        ("GIT_AUTHOR_EMAIL", email.as_str()),
        ("GIT_COMMITTER_NAME", name.as_str()),
        ("GIT_COMMITTER_EMAIL", email.as_str()),
    ];
    if out(&worktree, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_ok() {
        // A merge is already under way; leave it for the seat.
        return Ok(Vec::new());
    }
    let dirty = out(&worktree, &["status", "--porcelain"])?;
    if !dirty.is_empty() {
        let added = run(&worktree, &["add", "-A"], &who)?;
        if !added.status.success() {
            return Err(format!(
                "Git cannot stage the task's work: {}",
                reason(&added)
            ));
        }
        let message = format!("Work of studio task {}", &task[..task.len().min(12)]);
        let committed = run(
            &worktree,
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "--no-verify",
                "-m",
                &message,
            ],
            &who,
        )?;
        if !committed.status.success() {
            return Err(format!(
                "Git cannot commit the task's work: {}",
                reason(&committed)
            ));
        }
    }
    let merged = run(
        &worktree,
        &[
            "-c",
            "commit.gpgsign=false",
            "merge",
            "--no-ff",
            "--no-commit",
            target,
        ],
        &who,
    )?;
    let conflicted = out(&worktree, &["diff", "--name-only", "--diff-filter=U"])?;
    if !merged.status.success() && conflicted.is_empty() {
        return Err(format!("Git cannot merge {target}: {}", reason(&merged)));
    }
    Ok(conflicted.lines().map(str::to_owned).collect())
}

pub fn merge(store: &Path, task: &str, reviewed: &Reviewed) -> Result<Publication, Refusal> {
    let _one = MERGING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(done) =
        publish::find(store, task, reviewed).filter(|kept| kept.state == PublishState::Published)
    {
        return Ok(done);
    }
    let record = retire::ensure(store, task)
        .ok()
        .flatten()
        .ok_or(Refusal::NoWorktree)?;
    let worktree = PathBuf::from(&record.worktree);
    if !worktree.is_dir() {
        return Err(Refusal::NoWorktree);
    }
    let seat = seat_of(store, task).unwrap_or_else(|| "worker".into());
    let mut publication = Publication {
        operation: publish::operation_id(task, reviewed),
        task: task.into(),
        base: reviewed.base.clone(),
        head_commit: reviewed.head_commit.clone(),
        head: reviewed.head.clone(),
        landing: Landing::Branch,
        state: PublishState::Refused,
        branch: None,
        commit: None,
        url: None,
        note: String::new(),
    };
    let checkout = PathBuf::from(&record.checkout);
    if super::flow::stage_of(store, task) == Some(super::Stage::Review) {
        publication.note = "The lead is still reviewing this change; merge it once the review \
                            asks for your decision."
            .into();
        publish::remember(store, task, &publication)?;
        return Ok(publication);
    }
    let mut conflict = None;
    let signer = seat_signer(store);
    match land(
        &checkout,
        &worktree,
        task,
        &seat,
        &record.base,
        reviewed,
        signer.as_ref(),
        &mut conflict,
    ) {
        Ok(landed) => {
            publication.state = PublishState::Published;
            publication.note = clip(
                &if landed.already {
                    format!(
                        "{} already holds this change; nothing was pushed.",
                        landed.branch
                    )
                } else {
                    format!(
                        "Merged into {} in {} as {}; nothing was pushed.",
                        landed.branch,
                        checkout.display(),
                        &landed.commit[..12]
                    )
                },
                MAX_NOTE,
            );
            publication.branch = Some(landed.branch);
            publication.commit = Some(landed.commit);
        }
        Err(why) => publication.note = clip(&why, MAX_NOTE),
    }
    publish::remember(store, task, &publication)?;
    if let Some(conflict) = conflict
        && let Err(why) = super::flow::note_conflict(store, task, &conflict)
    {
        eprintln!("openagents host: studio: cannot note the conflict of task {task}: {why}");
    }
    // The merge holds the worktree's whole content: an ended task's
    // worktree then goes, as after a publication.
    if publication.state == PublishState::Published {
        let _ = retire::retire(store, task);
    }
    Ok(publication)
}

/// Signs a seat's commit: `(seat, worktree, commit)` to the signed
/// commit's ID, or `None` when the seat doesn't sign its commits.
pub type SeatSigner =
    Arc<dyn Fn(&str, &Path, &str) -> Option<Result<String, String>> + Send + Sync>;

/// Each task store's seat signer.
static SIGNERS: Mutex<BTreeMap<PathBuf, SeatSigner>> = Mutex::new(BTreeMap::new());

/// Makes `signer` sign the seats' commits that merges from task store
/// `store` land.
pub fn set_seat_signer(store: &Path, signer: SeatSigner) {
    SIGNERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(store.to_path_buf(), signer);
}

fn seat_signer(store: &Path) -> Option<SeatSigner> {
    SIGNERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(store)
        .cloned()
}

/// Where a merge landed.
struct Landed {
    branch: String,
    commit: String,
    /// The branch already held the change; nothing moved.
    already: bool,
}

/// Build the merge off-tree and fast-forward the checkout's branch to it.
/// The reason is a sentence the person reads; a merge that would conflict
/// also fills `conflict`. With `signer`, the tip of the seat's change is
/// signed as the seat before it is merged.
#[allow(clippy::too_many_arguments)]
fn land(
    checkout: &Path,
    worktree: &Path,
    task: &str,
    seat: &str,
    base: &str,
    reviewed: &Reviewed,
    signer: Option<&SeatSigner>,
    conflict: &mut Option<super::flow::Conflict>,
) -> Result<Landed, String> {
    let now = review::head(worktree)?;
    if now.commit != reviewed.head_commit || now.tree != reviewed.head {
        return Err("The task's worktree changed since the review; review it again.".into());
    }
    let base_tree = out(worktree, &["rev-parse", &format!("{base}^{{tree}}")])
        .map_err(|why| format!("Git cannot read the task's base: {why}"))?;
    if base_tree == reviewed.head {
        return Err("The task changed nothing, so there is nothing to merge.".into());
    }
    // Work the task left uncommitted is committed as the seat, off its
    // branch, so the merge holds exactly the reviewed tree.
    let head_tree = out(
        worktree,
        &["rev-parse", &format!("{}^{{tree}}", reviewed.head_commit)],
    )?;
    let change = if head_tree == reviewed.head {
        reviewed.head_commit.clone()
    } else {
        let (name, email) = identity(seat);
        let message = format!(
            "Uncommitted work of studio task {}",
            &task[..task.len().min(12)]
        );
        // A merge the host started for a conflict (`resolve_conflict`) and
        // the seat resolved keeps the merged branch as its second parent.
        let merging = out(worktree, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).ok();
        let mut arguments = vec!["commit-tree", &reviewed.head, "-p", &reviewed.head_commit];
        if let Some(other) = &merging {
            arguments.extend(["-p", other.as_str()]);
        }
        arguments.extend(["-m", &message]);
        let output = run(
            worktree,
            &arguments,
            &[
                ("GIT_AUTHOR_NAME", name.as_str()),
                ("GIT_AUTHOR_EMAIL", email.as_str()),
                ("GIT_COMMITTER_NAME", name.as_str()),
                ("GIT_COMMITTER_EMAIL", email.as_str()),
            ],
        )?;
        if !output.status.success() {
            return Err(format!(
                "Git cannot commit the task's uncommitted work: {}",
                reason(&output)
            ));
        }
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    let change = match signer.and_then(|sign| sign(seat, worktree, &change)) {
        None => change,
        Some(Ok(signed)) => signed,
        Some(Err(why)) => {
            return Err(format!(
                "{seat}'s change can't be signed with her key, so nothing merged: {why}"
            ));
        }
    };
    let dirty =
        out(checkout, &["status", "--porcelain", "--untracked-files=no"]).map_err(|why| {
            format!(
                "Git cannot read the checkout at {}: {why}",
                checkout.display()
            )
        })?;
    if !dirty.is_empty() {
        return Err(format!(
            "The checkout at {} has uncommitted changes. Commit or stash them, then merge again.",
            checkout.display()
        ));
    }
    let target = out(checkout, &["symbolic-ref", "-q", "--short", "HEAD"]).map_err(|_| {
        format!(
            "The checkout at {} is not on a branch. Check out the branch to merge into, then merge again.",
            checkout.display()
        )
    })?;
    let tip = out(checkout, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let contained = run(
        checkout,
        &["merge-base", "--is-ancestor", &change, &tip],
        &[],
    )?;
    if contained.status.success() {
        return Ok(Landed {
            branch: target,
            commit: tip,
            already: true,
        });
    }
    let merged = run(
        checkout,
        &[
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            &tip,
            &change,
        ],
        &[],
    )?;
    let listing = String::from_utf8_lossy(&merged.stdout).into_owned();
    let mut lines = listing.lines().map(str::trim);
    let tree = lines.next().unwrap_or_default().to_owned();
    match merged.status.code() {
        Some(0) => {}
        Some(1) => {
            let files: Vec<&str> = lines.take_while(|line| !line.is_empty()).collect();
            *conflict = Some(super::flow::Conflict {
                target: target.clone(),
                files: files
                    .iter()
                    .take(super::flow::MAX_CONFLICT_FILES)
                    .map(|file| (*file).to_owned())
                    .collect(),
                head_commit: reviewed.head_commit.clone(),
            });
            return Err(format!(
                "Merging would conflict in {}. Nothing changed; resolve it in the task, then merge again.",
                if files.is_empty() {
                    "files Git did not name".to_owned()
                } else {
                    files.join(", ")
                }
            ));
        }
        _ => return Err(format!("Git cannot merge the change: {}", reason(&merged))),
    }
    let studio_branch = out(worktree, &["symbolic-ref", "-q", "--short", "HEAD"])
        .unwrap_or_else(|_| format!("studio task {}", &task[..task.len().min(12)]));
    let message = format!(
        "Merge {studio_branch} into {target}\n\nApproved in the Agent Studio for task {task}."
    );
    // The person's own identity, from their Git configuration; signed when
    // it signs commits, which `commit-tree` does not read by itself.
    let sign = out(
        checkout,
        &["config", "--type=bool", "--get", "commit.gpgsign"],
    )
    .is_ok_and(|value| value == "true");
    let mut args = vec!["commit-tree"];
    if sign {
        args.push("-S");
    }
    args.extend([
        tree.as_str(),
        "-p",
        tip.as_str(),
        "-p",
        change.as_str(),
        "-m",
        message.as_str(),
    ]);
    let made = run(checkout, &args, &[])?;
    if !made.status.success() {
        return Err(if sign {
            format!(
                "Git cannot sign the merge commit, as your configuration asks: {}",
                reason(&made)
            )
        } else {
            format!("Git cannot make the merge commit: {}", reason(&made))
        });
    }
    let commit = String::from_utf8_lossy(&made.stdout).trim().to_owned();
    let moved = run(checkout, &["merge", "--ff-only", "-q", &commit], &[])?;
    if !moved.status.success() {
        return Err(format!(
            "Git cannot fast-forward {target} in {}: {}",
            checkout.display(),
            reason(&moved)
        ));
    }
    Ok(Landed {
        branch: target,
        commit,
        already: false,
    })
}

#[cfg(test)]
#[path = "studio_git_tests.rs"]
mod tests;
