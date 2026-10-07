//! Submitting a change and running an artifact's queue.

use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{
    Artifact, Queue, REF_PREFIX, SUBMISSION_SCHEMA, Status, Submission, apply_pin, join_summaries,
    new_id, reset_pin,
};
use crate::{Broker, Error, Holder, Request, Resource, Wait};

/// How many lines of a failing command's output a rejection keeps.
const TAIL_LINES: usize = 20;

/// Where the queue fetches from and pushes to.
#[derive(Clone, Debug)]
pub struct Options {
    /// A checkout of the repository: the submitter's, or the one the
    /// runner makes its scratch worktree from.
    pub repository: PathBuf,
    /// The remote, `origin` by default.
    pub remote: String,
    /// The branch the queue lands on, `main` by default.
    pub branch: String,
    /// Variables added to every Git and artifact command's environment.
    pub env: Vec<(OsString, OsString)>,
    /// How many times a batch is rebuilt on a fresh `main` after a refused
    /// push.
    pub attempts: u32,
}

impl Options {
    /// Options for the checkout at `repository`: `origin`, `main`, and
    /// three attempts.
    #[must_use]
    pub fn new(repository: PathBuf) -> Options {
        Options {
            repository,
            remote: "origin".to_owned(),
            branch: "main".to_owned(),
            env: Vec::new(),
            attempts: 3,
        }
    }

    fn upstream(&self) -> String {
        format!("{}/{}", self.remote, self.branch)
    }
}

/// What a run of the queue did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Another process holds the artifact's lease and runs the queue.
    Busy {
        /// The holder's session.
        session: String,
        /// The holder's process.
        pid: u32,
    },
    /// This process ran the queue until it was empty.
    Ran {
        /// The changes that landed.
        landed: Vec<Submission>,
        /// The changes sent back, with their reasons.
        rejected: Vec<Submission>,
    },
}

/// Queues the commit `branch` names (the checked-out branch when `None`)
/// as a change to artifact `name`, from the checkout `options.repository`.
///
/// # Errors
/// A sentence when the artifact is not in the checkout's registry, the
/// branch can't be resolved or has nothing `main` lacks, or the record
/// can't be written.
pub fn submit(
    root: &Path,
    name: &str,
    branch: Option<&str>,
    summary: Option<&str>,
    session: &str,
    options: &Options,
) -> Result<Submission, String> {
    let checkout = &options.repository;
    let top = PathBuf::from(git(checkout, &["rev-parse", "--show-toplevel"], options)?.trim());
    Artifact::load(&top, name)?;
    let repository = git_common_dir(checkout, options)?;
    let branch = match branch {
        Some(branch) => branch.to_owned(),
        None => git(checkout, &["symbolic-ref", "--short", "HEAD"], options)
            .map_err(|_| "HEAD is detached; name the change with --branch".to_owned())?
            .trim()
            .to_owned(),
    };
    let commit = git(
        checkout,
        &["rev-parse", "--verify", &format!("{branch}^{{commit}}")],
        options,
    )
    .map_err(|_| format!("`{branch}` is not a branch or commit here"))?
    .trim()
    .to_owned();
    let upstream = options.upstream();
    let ahead = git(
        checkout,
        &["rev-list", "--count", &format!("{upstream}..{commit}")],
        options,
    )?;
    if ahead.trim() == "0" {
        return Err(format!("{branch} has no commits that {upstream} lacks"));
    }
    let id = new_id();
    let git_ref = format!("{REF_PREFIX}/{name}/{id}");
    git(checkout, &["update-ref", &git_ref, &commit], options)?;
    let submission = Submission {
        schema: SUBMISSION_SCHEMA.to_owned(),
        id,
        artifact: name.to_owned(),
        repository,
        summary: summary
            .filter(|summary| !summary.trim().is_empty())
            .map_or_else(|| branch.clone(), |summary| summary.trim().to_owned()),
        branch,
        commit,
        git_ref,
        session: session.to_owned(),
        submitted_at_ms: crate::now_ms(),
        status: Status::Pending,
        reason: None,
        landed: None,
        finished_at_ms: None,
    };
    Queue::new(root, name).save(&submission)?;
    Ok(submission)
}

/// Runs artifact `name`'s queue under its exclusive lease until no change
/// is pending, or reports who already runs it.
///
/// # Errors
/// A sentence when the lease table, the scratch worktree, the registry,
/// or the push fails in a way no single change caused. Changes not yet
/// decided stay pending.
pub fn run(broker: &Broker, name: &str, options: &Options) -> Result<Outcome, String> {
    let queue = Queue::new(broker.root(), name);
    let mut landed = Vec::new();
    let mut rejected = Vec::new();
    let mut first = true;
    loop {
        let request = Request::new(
            Resource::Artifact(name.to_owned()),
            Holder::detect("artifact"),
        )
        .wait(Wait::No);
        let lease = match broker.acquire(request) {
            Ok(lease) => lease,
            Err(Error::Busy(blocked)) => {
                if first && let Some(holder) = blocked.by.first() {
                    return Ok(Outcome::Busy {
                        session: holder.holder.session.clone(),
                        pid: holder.holder.pid,
                    });
                }
                // Another runner took the lease after this one let go; it
                // applies whatever is still pending.
                break;
            }
            Err(error) => return Err(error.to_string()),
        };
        first = false;
        let mut runner = Runner {
            queue: &queue,
            broker,
            options,
            landed: &mut landed,
            rejected: &mut rejected,
        };
        let result = runner.drain();
        let _ = lease.release(Some(i32::from(result.is_err())));
        result?;
        // A change submitted while the lease was held, after the last look
        // at the queue, found the lease taken; look once more now that it
        // is free.
        if queue.pending()?.is_empty() {
            break;
        }
    }
    Ok(Outcome::Ran { landed, rejected })
}

/// The Git directory every worktree of the checkout at `dir` shares.
///
/// # Errors
/// A sentence when `dir` is not in a Git checkout.
pub fn git_common_dir(dir: &Path, options: &Options) -> Result<PathBuf, String> {
    let out = git(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        options,
    )
    .map_err(|error| format!("{} is not in a Git checkout: {error}", dir.display()))?;
    Ok(PathBuf::from(out.trim()))
}

/// A change applied in the scratch worktree.
struct Picked {
    submission: Submission,
    /// The change as one commit on its merge base, pinned files left out.
    commit: String,
}

struct Runner<'a> {
    queue: &'a Queue,
    broker: &'a Broker,
    options: &'a Options,
    landed: &'a mut Vec<Submission>,
    rejected: &'a mut Vec<Submission>,
}

impl Runner<'_> {
    fn drain(&mut self) -> Result<(), String> {
        while !self.queue.pending()?.is_empty() {
            self.batch()?;
        }
        Ok(())
    }

    /// Applies every pending change onto a fresh `main`, repins once,
    /// checks, and pushes; rebuilds the batch when the push is refused.
    fn batch(&mut self) -> Result<(), String> {
        let attempts = self.options.attempts.max(1);
        for _ in 0..attempts {
            let worktree = self.worktree()?;
            let artifact = Artifact::load(&worktree, self.queue.name())?;
            let base = rev(&worktree, "HEAD", self.options)?;
            let mut picked = Vec::new();
            for submission in self.queue.pending()? {
                match self.prepare(&worktree, &artifact, &submission) {
                    Ok(commit) => match self.pick(&worktree, &commit) {
                        Ok(()) => picked.push(Picked { submission, commit }),
                        Err(reason) => self.reject(submission, &reason)?,
                    },
                    Err(reason) => self.reject(submission, &reason)?,
                }
            }
            if picked.is_empty() {
                return Ok(());
            }
            let accepted = match self.regenerate_and_check(&worktree, &artifact) {
                Ok(()) => picked,
                Err(reason) if picked.len() == 1 => {
                    let only = picked.remove(0);
                    return self.reject(only.submission, &reason);
                }
                Err(_) => {
                    let accepted = self.one_at_a_time(&worktree, &artifact, &base, picked)?;
                    if accepted.is_empty() {
                        return Ok(());
                    }
                    if let Err(reason) = self.regenerate_and_check(&worktree, &artifact) {
                        for one in accepted {
                            self.reject(one.submission, &reason)?;
                        }
                        return Ok(());
                    }
                    accepted
                }
            };
            self.commit_repin(&worktree, &artifact, &accepted)?;
            let pushed = git(
                &worktree,
                &[
                    "push",
                    "--quiet",
                    &self.options.remote,
                    &format!("HEAD:refs/heads/{}", self.options.branch),
                ],
                self.options,
            );
            if pushed.is_ok() {
                let head = rev(&worktree, "HEAD", self.options)?;
                for one in accepted {
                    self.finish(one.submission, Status::Landed, None, Some(head.clone()))?;
                }
                return Ok(());
            }
        }
        Err(format!(
            "{} refused the push {attempts} times; the changes stay pending",
            self.options.upstream()
        ))
    }

    /// After the whole batch failed its check: applies the changes one at
    /// a time, regenerating and checking after each, and rejects the ones
    /// that fail. Leaves the worktree with the accepted changes applied.
    fn one_at_a_time(
        &mut self,
        worktree: &Path,
        artifact: &Artifact,
        base: &str,
        picked: Vec<Picked>,
    ) -> Result<Vec<Picked>, String> {
        let mut good = base.to_owned();
        self.reset(worktree, &good)?;
        let mut accepted = Vec::new();
        for one in picked {
            if let Err(reason) = self.pick(worktree, &one.commit) {
                self.reject(one.submission, &reason)?;
                continue;
            }
            match self.regenerate_and_check(worktree, artifact) {
                Ok(()) => {
                    good = rev(worktree, "HEAD", self.options)?;
                    self.reset(worktree, &good)?;
                    accepted.push(one);
                }
                Err(reason) => {
                    self.reset(worktree, &good)?;
                    self.reject(one.submission, &reason)?;
                }
            }
        }
        Ok(accepted)
    }

    /// The scratch worktree, at a fresh `main` with nothing else in it.
    fn worktree(&self) -> Result<PathBuf, String> {
        let options = self.options;
        let worktree = self.queue.dir().join("worktree");
        if !worktree.join(".git").exists() {
            if worktree.exists() {
                std::fs::remove_dir_all(&worktree)
                    .map_err(|error| format!("{}: {error}", worktree.display()))?;
            }
            std::fs::create_dir_all(self.queue.dir())
                .map_err(|error| format!("{}: {error}", self.queue.dir().display()))?;
            let _ = git(&options.repository, &["worktree", "prune"], options);
            let path = worktree.to_string_lossy();
            git(
                &options.repository,
                &["worktree", "add", "--quiet", "--detach", "--force", &path],
                options,
            )?;
        }
        let _ = git(&worktree, &["cherry-pick", "--abort"], options);
        git(
            &worktree,
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                &options.remote,
                &options.branch,
            ],
            options,
        )
        .map_err(|error| format!("{} could not be fetched: {error}", options.upstream()))?;
        self.reset(&worktree, "FETCH_HEAD")?;
        Ok(worktree)
    }

    fn reset(&self, worktree: &Path, to: &str) -> Result<(), String> {
        git(worktree, &["reset", "--quiet", "--hard", to], self.options)?;
        git(worktree, &["clean", "-fdxq"], self.options)?;
        Ok(())
    }

    /// The submission as one commit on its merge base with `main`, with
    /// its edits to pinned files and pin lines left out.
    fn prepare(
        &self,
        worktree: &Path,
        artifact: &Artifact,
        submission: &Submission,
    ) -> Result<String, String> {
        let options = self.options;
        let source = submission.repository.to_string_lossy();
        git(
            worktree,
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                &source,
                &submission.git_ref,
            ],
            options,
        )
        .map_err(|error| {
            format!(
                "the queue could not fetch {} from {source}: {error}",
                submission.git_ref
            )
        })?;
        let commit = &submission.commit;
        if rev(worktree, "FETCH_HEAD", options)? != *commit {
            return Err(format!(
                "{} no longer names the submitted commit",
                submission.git_ref
            ));
        }
        let merge_base = git(worktree, &["merge-base", "HEAD", commit], options)
            .map_err(|_| format!("{} shares no history with main", submission.branch))?
            .trim()
            .to_owned();
        if merge_base == *commit {
            return Err(format!(
                "{} is already in {}",
                submission.branch,
                options.upstream()
            ));
        }
        let tree = self.unpinned_tree(worktree, artifact, &merge_base, commit)?;
        if tree == rev(worktree, &format!("{merge_base}^{{tree}}"), options)? {
            return Err(
                "the change has nothing to apply once its pinned files are left out".to_owned(),
            );
        }
        let count = git(
            worktree,
            &["rev-list", "--count", &format!("{merge_base}..{commit}")],
            options,
        )?;
        let message = if count.trim() == "1" {
            git(worktree, &["log", "-1", "--format=%B", commit], options)?
        } else {
            let subjects = git(
                worktree,
                &[
                    "log",
                    "--reverse",
                    "--format=- %s",
                    &format!("{merge_base}..{commit}"),
                ],
                options,
            )?;
            format!(
                "Apply {} through the {} queue\n\n{}",
                submission.summary, artifact.name, subjects
            )
        };
        let author = git(
            worktree,
            &["log", "-1", "--format=%an%n%ae%n%aI", commit],
            options,
        )?;
        let mut author = author.lines();
        let mut env = vec![];
        for var in ["GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_AUTHOR_DATE"] {
            env.push((
                OsString::from(var),
                OsString::from(author.next().unwrap_or("")),
            ));
        }
        let made = git_with(
            worktree,
            &["commit-tree", &tree, "-p", &merge_base, "-F", "-"],
            options,
            &env,
            Some(message.trim().as_bytes()),
        )?;
        Ok(made.trim().to_owned())
    }

    /// The tree of `commit` with every pinned file it changed put back as
    /// `merge_base` has it, and the pin lines of the pin file too.
    fn unpinned_tree(
        &self,
        worktree: &Path,
        artifact: &Artifact,
        merge_base: &str,
        commit: &str,
    ) -> Result<String, String> {
        let options = self.options;
        let index = self.queue.dir().join("index.tmp");
        let _ = std::fs::remove_file(&index);
        let env = vec![(
            OsString::from("GIT_INDEX_FILE"),
            index.clone().into_os_string(),
        )];
        let run =
            |args: &[&str], input: Option<&[u8]>| git_with(worktree, args, options, &env, input);
        run(&["read-tree", commit], None)?;
        let changed = git(
            worktree,
            &[
                "diff",
                "--name-only",
                "-z",
                "--no-renames",
                merge_base,
                commit,
            ],
            options,
        )?;
        for path in changed.split('\0').filter(|path| !path.is_empty()) {
            if artifact.pinned(path) {
                match entry(worktree, merge_base, path, options)? {
                    Some((mode, object)) => {
                        run(
                            &[
                                "update-index",
                                "--add",
                                "--cacheinfo",
                                &format!("{mode},{object},{path}"),
                            ],
                            None,
                        )?;
                    }
                    None => {
                        run(&["update-index", "--force-remove", "--", path], None)?;
                    }
                }
            } else if let Some(pin) = &artifact.pin
                && pin.file == path
                && let (Some(_), Some((mode, object))) = (
                    entry(worktree, merge_base, path, options)?,
                    entry(worktree, commit, path, options)?,
                )
            {
                let changed = git(worktree, &["cat-file", "blob", &object], options)?;
                let base = git(
                    worktree,
                    &["cat-file", "blob", &format!("{merge_base}:{path}")],
                    options,
                )?;
                let reset = reset_pin(pin, &changed, &base);
                if reset != changed {
                    let blob = git_with(
                        worktree,
                        &["hash-object", "-w", "--stdin"],
                        options,
                        &[],
                        Some(reset.as_bytes()),
                    )?;
                    run(
                        &[
                            "update-index",
                            "--add",
                            "--cacheinfo",
                            &format!("{mode},{},{path}", blob.trim()),
                        ],
                        None,
                    )?;
                }
            }
        }
        let tree = run(&["write-tree"], None)?;
        let _ = std::fs::remove_file(&index);
        Ok(tree.trim().to_owned())
    }

    /// Cherry-picks a prepared change; on a conflict, aborts and names the
    /// conflicting files.
    fn pick(&self, worktree: &Path, commit: &str) -> Result<(), String> {
        let options = self.options;
        match git(
            worktree,
            &[
                "cherry-pick",
                "--allow-empty",
                "--keep-redundant-commits",
                commit,
            ],
            options,
        ) {
            Ok(_) => Ok(()),
            Err(error) => {
                let conflicts = git(
                    worktree,
                    &["diff", "--name-only", "--diff-filter=U"],
                    options,
                )
                .unwrap_or_default();
                let _ = git(worktree, &["cherry-pick", "--abort"], options);
                let files: Vec<&str> = conflicts.lines().filter(|l| !l.is_empty()).collect();
                if files.is_empty() {
                    Err(format!(
                        "the change does not apply onto {}: {error}",
                        options.upstream()
                    ))
                } else {
                    Err(format!(
                        "the change conflicts with {} or an earlier change in the batch in {}; rebase it and submit again",
                        options.upstream(),
                        files.join(", ")
                    ))
                }
            }
        }
    }

    /// Runs the artifact's regenerate command, writes the pin lines it
    /// printed, and runs its check, under a `build` lease when the
    /// artifact asks for one.
    fn regenerate_and_check(&self, worktree: &Path, artifact: &Artifact) -> Result<(), String> {
        let lease = if artifact.build_lease {
            let request = Request::new(Resource::Build, Holder::detect("artifact")).inherit_env();
            Some(
                self.broker
                    .acquire(request)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        let mut env: Vec<(OsString, OsString)> = lease
            .as_ref()
            .map(|lease| {
                lease
                    .env()
                    .into_iter()
                    .map(|(name, value)| (name.into(), value.into()))
                    .collect()
            })
            .unwrap_or_default();
        let target_set = std::env::var_os("CARGO_TARGET_DIR").is_some()
            || self
                .options
                .env
                .iter()
                .any(|(name, _)| name == "CARGO_TARGET_DIR");
        if !target_set {
            env.push((
                "CARGO_TARGET_DIR".into(),
                self.queue.dir().join("target").into_os_string(),
            ));
        }
        let result = (|| {
            let output = shell(worktree, &artifact.regenerate, self.options, &env)
                .map_err(|tail| format!("regenerating {} failed:\n{tail}", artifact.name))?;
            if let Some(pin) = &artifact.pin {
                let path = worktree.join(&pin.file);
                let current = std::fs::read_to_string(&path)
                    .map_err(|error| format!("{}: {error}", pin.file))?;
                let next = apply_pin(pin, &current, &output)?;
                if next != current {
                    std::fs::write(&path, next)
                        .map_err(|error| format!("{}: {error}", pin.file))?;
                }
            }
            shell(worktree, &artifact.check, self.options, &env)
                .map_err(|tail| format!("the check failed:\n{tail}"))?;
            Ok(())
        })();
        if let Some(lease) = lease {
            let _ = lease.release(Some(i32::from(result.is_err())));
        }
        result
    }

    /// Commits what regenerating changed, when it changed anything.
    fn commit_repin(
        &self,
        worktree: &Path,
        artifact: &Artifact,
        accepted: &[Picked],
    ) -> Result<(), String> {
        let options = self.options;
        git(worktree, &["add", "--all"], options)?;
        if git(worktree, &["status", "--porcelain"], options)?
            .trim()
            .is_empty()
        {
            return Ok(());
        }
        let summaries: Vec<String> = accepted
            .iter()
            .map(|one| one.submission.summary.clone())
            .collect();
        let subject = artifact
            .message
            .replace("{changes}", &join_summaries(&summaries));
        let mut body = format!(
            "The {} queue applied these changes onto {} and regenerated the pin once:\n",
            artifact.name,
            options.upstream()
        );
        for one in accepted {
            body.push_str(&format!(
                "\n- {} ({}, from session {})",
                one.submission.summary, one.submission.branch, one.submission.session
            ));
        }
        let message = format!("{subject}\n\n{body}\n");
        git_with(
            worktree,
            &["commit", "--quiet", "-F", "-"],
            options,
            &[],
            Some(message.as_bytes()),
        )?;
        Ok(())
    }

    fn reject(&mut self, submission: Submission, reason: &str) -> Result<(), String> {
        self.finish(submission, Status::Rejected, Some(reason.to_owned()), None)
    }

    fn finish(
        &mut self,
        mut submission: Submission,
        status: Status,
        reason: Option<String>,
        landed: Option<String>,
    ) -> Result<(), String> {
        submission.status = status;
        submission.reason = reason;
        submission.landed = landed;
        submission.finished_at_ms = Some(crate::now_ms());
        self.queue.save(&submission)?;
        let _ = git_dir(
            &submission.repository,
            &["update-ref", "-d", &submission.git_ref],
            self.options,
        );
        match status {
            Status::Landed => self.landed.push(submission),
            Status::Rejected => self.rejected.push(submission),
            Status::Pending => {}
        }
        Ok(())
    }
}

/// The mode and object of `path` in `commit`, when it is there.
fn entry(
    worktree: &Path,
    commit: &str,
    path: &str,
    options: &Options,
) -> Result<Option<(String, String)>, String> {
    let line = git(worktree, &["ls-tree", "-z", commit, "--", path], options)?;
    let line = line.trim_end_matches('\0');
    if line.is_empty() {
        return Ok(None);
    }
    let meta = line.split('\t').next().unwrap_or_default();
    let mut parts = meta.split(' ');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(mode), Some(_), Some(object)) => Ok(Some((mode.to_owned(), object.to_owned()))),
        _ => Err(format!("git ls-tree printed `{line}`")),
    }
}

fn rev(dir: &Path, what: &str, options: &Options) -> Result<String, String> {
    Ok(git(dir, &["rev-parse", "--verify", what], options)?
        .trim()
        .to_owned())
}

fn git(dir: &Path, args: &[&str], options: &Options) -> Result<String, String> {
    git_with(dir, args, options, &[], None)
}

/// Git in a Git directory rather than a checkout.
fn git_dir(git_dir: &Path, args: &[&str], options: &Options) -> Result<String, String> {
    let mut all = vec![OsString::from("--git-dir"), git_dir.as_os_str().to_owned()];
    all.extend(args.iter().map(OsString::from));
    let mut command = Command::new("git");
    command.args(&all).envs(options.env.iter().cloned());
    finish_git(command, args, None)
}

fn git_with(
    dir: &Path,
    args: &[&str],
    options: &Options,
    env: &[(OsString, OsString)],
    input: Option<&[u8]>,
) -> Result<String, String> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(args)
        .envs(options.env.iter().cloned())
        .envs(env.iter().cloned());
    finish_git(command, args, input)
}

fn finish_git(mut command: Command, args: &[&str], input: Option<&[u8]>) -> Result<String, String> {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("git could not start: {error}"))?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin
            .write_all(input)
            .map_err(|error| format!("git {}: {error}", args.join(" ")))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("git {}: {error}", args.join(" ")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Runs `script` with `sh -c` in `dir`; its standard output on success,
/// else the last lines of its output.
fn shell(
    dir: &Path,
    script: &str,
    options: &Options,
    env: &[(OsString, OsString)],
) -> Result<String, String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .envs(options.env.iter().cloned())
        .envs(env.iter().cloned())
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("sh could not start: {error}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let lines: Vec<&str> = text.lines().collect();
    let tail = lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n");
    Err(format!("`{script}` exited with {}\n{tail}", output.status))
}
