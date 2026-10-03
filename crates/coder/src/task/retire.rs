//! Removing a task's worktree once the task ends, and recreating it when
//! the task is used again (#10291).
//!
//! A local run works in a detached worktree of its own under `worktrees/`
//! beside the task store ([`super::local`]). Nothing removed one when its
//! task ended, so finished tasks' worktrees filled hosts' disks. Now, when
//! a task ends — its turn finished or was stopped, its checks are over, no
//! issue flow is still checking or landing it — and its worktree holds
//! nothing unsaved, the worktree is removed and pruned at once:
//!
//! - **landed** — an issue flow committed and pushed the change, so the
//!   worktree is clean and its commit is on a remote;
//! - **pull request opened / pushed** — a publication ([`super::publish`])
//!   pushed a commit of exactly the worktree's content;
//! - **stopped or finished clean** — the turn changed nothing.
//!
//! The check is the background cleaner's own ([`background::git`]): never
//! a worktree with uncommitted changes (unless a pushed publication holds
//! exactly them), ignored files outside build caches, commits on no
//! remote, or a stash made on it. A worktree with any of those stays.
//!
//! A follow-up continues the same task ([`super::local::Local::answer`]),
//! so a removed worktree is not gone for good: the run's record keeps the
//! commit ([`Archived`]), and anything that uses the worktree again first
//! recreates it, detached at that commit, at the same path ([`restore`]).
//! Removal and restore of one task take one lock ([`lock`]), which a
//! follow-up holds from before it reads the record until its turn has
//! started, so a removal never races a turn that is starting.

use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::local::{self, Record};
use super::{Status, Store};

/// What recreates a task's removed worktree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Archived {
    /// The commit the worktree is recreated at: its `HEAD`, or the
    /// published commit that held its content.
    pub commit: String,
    /// The branch it was on, if it was not detached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// When it was removed, in seconds since the Unix epoch.
    pub removed_at: u64,
}

/// What [`retire`] did with a task's worktree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Retired {
    /// Removed; the record says how to recreate it.
    Removed,
    /// Kept, and why: it holds something unsaved, or Git refused.
    Kept(String),
    /// The task has not ended: a turn, its checks, or its issue flow
    /// still runs.
    NotEnded,
    /// Nothing to do: not a local run, already removed, or never made.
    Absent,
}

fn lock_path(store: &Path, task: &str) -> PathBuf {
    store.join("local").join(format!("{task}.worktree.lock"))
}

/// The lock of `task`'s worktree: held while it is removed or recreated,
/// and by a follow-up from reading the record until its turn started.
///
/// # Errors
/// The lock file cannot be made or locked.
pub fn lock(store: &Path, task: &str) -> Result<File, String> {
    let path = lock_path(store, task);
    if let Some(parent) = path.parent() {
        crate::private::create_dir_all(parent)
            .map_err(|_| format!("cannot create {}", parent.display()))?;
    }
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    file.lock()
        .map_err(|error| format!("cannot lock {}: {error}", path.display()))?;
    Ok(file)
}

/// Remove `task`'s worktree when the task has ended and the worktree holds
/// nothing unsaved; see the module docs.
pub fn retire(store: &Path, task: &str) -> Retired {
    let Ok(_held) = lock(store, task) else {
        return Retired::Kept("its worktree lock cannot be taken".into());
    };
    let Some(mut record) = local::record(store, task) else {
        return Retired::Absent;
    };
    let worktree = PathBuf::from(&record.worktree);
    if record.archived.is_some() || !worktree.exists() {
        return Retired::Absent;
    }
    // Never the person's own checkout.
    if Path::new(&record.checkout) == worktree {
        return Retired::Kept("it is the checkout itself".into());
    }
    let Ok(current) = Store::open_waiting(store, std::time::Duration::from_secs(30))
        .and_then(|tasks| tasks.show(task))
    else {
        return Retired::Kept("the task store cannot be read".into());
    };
    if !current.ended() || !matches!(current.status, Status::Finished | Status::Cancelled) {
        return Retired::NotEnded;
    }
    if super::issue_run::load(store, task).is_some_and(|flow| !flow.finished) {
        return Retired::NotEnded;
    }
    let published = super::publish::last(store, task).and_then(|publication| {
        use coder_host::access::review::PublishState;
        matches!(
            publication.state,
            PublishState::Published | PublishState::Pushed
        )
        .then_some(publication.commit)
        .flatten()
    });
    let checked = background::git::removable(&worktree).or_else(|why| match &published {
        Some(commit) => background::git::removable_with(&worktree, Some(commit)),
        None => Err(why),
    });
    let undo = match checked {
        Ok(undo) => undo,
        Err(why) => return Retired::Kept(why),
    };
    // The repository's own teardown first (#10297); one that fails keeps
    // the worktree, so nothing it would have stopped or saved is lost.
    if let Err(why) = super::worktree_hooks::teardown(
        Path::new(&record.checkout),
        &worktree,
        &local::hooks_log(store, task, "teardown"),
    ) {
        return Retired::Kept(format!("its teardown failed: {why}"));
    }
    let removed = if undo.commit == git_head(&worktree).unwrap_or_default() {
        background::git::remove(&undo)
    } else {
        background::git::remove_published(&undo)
    };
    if let Err(why) = removed {
        return Retired::Kept(why);
    }
    record.archived = Some(Archived {
        commit: undo.commit,
        branch: undo.branch,
        removed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
    });
    match local::save(store, &record) {
        Ok(()) => Retired::Removed,
        Err(why) => Retired::Kept(format!("removed, but the record was not saved: {why}")),
    }
}

fn git_head(worktree: &Path) -> Option<String> {
    local::git_out(worktree, &["rev-parse", "HEAD"])
        .ok()
        .map(|head| head.trim().to_owned())
}

/// Recreate `record`'s removed worktree at its path, detached at the
/// commit it was removed at (on its branch when it had one), and clear
/// [`Record::archived`]. Does nothing for a worktree that was never
/// removed. The caller holds [`lock`].
///
/// # Errors
/// Git cannot recreate it, or the record cannot be saved.
pub fn restore(store: &Path, record: &mut Record) -> Result<(), String> {
    let Some(archived) = record.archived.clone() else {
        return Ok(());
    };
    let worktree = PathBuf::from(&record.worktree);
    if !worktree.exists() {
        background::git::restore(&background::git::Undo {
            repo: PathBuf::from(&record.checkout),
            path: worktree,
            branch: archived.branch.clone(),
            commit: archived.commit.clone(),
        })
        .map_err(|why| format!("Coder could not recreate the task's worktree: {why}"))?;
        // A recreated worktree is made ready as a fresh one is (#10297).
        let ran = super::worktree_hooks::setup(
            Path::new(&record.checkout),
            Path::new(&record.worktree),
            &archived.commit,
            &local::hooks_log(store, &record.task, "setup"),
        )?;
        if ran != super::worktree_hooks::Ran::default() {
            record.hooks = Some(ran);
        }
    }
    record.archived = None;
    local::save(store, record)
}

/// [`restore`] under the task's lock, for a reader that needs the
/// worktree on disk: a review, or a plugin draft.
///
/// # Errors
/// As [`restore`] and [`lock`].
pub fn ensure(store: &Path, task: &str) -> Result<Option<Record>, String> {
    let _held = lock(store, task)?;
    let Some(mut record) = local::record(store, task) else {
        return Ok(None);
    };
    restore(store, &mut record)?;
    Ok(Some(record))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{
        Action, COMMAND_SCHEMA, Command, RequestedConfiguration, TaskIntent, Workspace,
    };

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = local::git().arg("-C").arg(dir).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// A checkout with a bare `origin`, a task store, and a local run's
    /// record whose worktree is detached at the pushed `main`.
    struct Scratch {
        _root: tempfile::TempDir,
        checkout: PathBuf,
        store: PathBuf,
        worktree: PathBuf,
        task: String,
        base: String,
    }

    impl Scratch {
        fn new() -> Self {
            Self::with_hooks(None)
        }

        /// [`Scratch::new`], with the repository's worktree hooks
        /// committed (#10297).
        fn with_hooks(hooks: Option<&str>) -> Self {
            let root = tempfile::tempdir().unwrap();
            let top = root.path().canonicalize().unwrap();
            let remote = top.join("remote.git");
            let checkout = top.join("checkout");
            std::fs::create_dir_all(&checkout).unwrap();
            git(
                &top,
                &[
                    "init",
                    "-q",
                    "--bare",
                    "-b",
                    "main",
                    remote.to_str().unwrap(),
                ],
            );
            git(&checkout, &["init", "-q", "-b", "main"]);
            for (key, value) in [
                ("user.email", "t@t"),
                ("user.name", "t"),
                ("commit.gpgsign", "false"),
                ("core.hooksPath", "/dev/null"),
            ] {
                git(&checkout, &["config", key, value]);
            }
            git(
                &checkout,
                &["remote", "add", "origin", remote.to_str().unwrap()],
            );
            std::fs::write(checkout.join("a.txt"), "one\n").unwrap();
            std::fs::write(checkout.join(".gitignore"), "target/\nsecret.env\n").unwrap();
            if let Some(hooks) = hooks {
                std::fs::create_dir_all(checkout.join(".openagents")).unwrap();
                std::fs::write(checkout.join(".openagents/worktree.json"), hooks).unwrap();
            }
            git(&checkout, &["add", "-A"]);
            git(&checkout, &["commit", "-q", "-m", "start"]);
            git(&checkout, &["push", "-q", "origin", "main"]);
            let base = git(&checkout, &["rev-parse", "HEAD"]);
            let worktree = top.join("worktrees").join("checkout-task");
            git(
                &checkout,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "--detach",
                    worktree.to_str().unwrap(),
                    "HEAD",
                ],
            );
            let store = top.join("store");
            let task = "5".repeat(64);
            let record = Record {
                schema: local::RECORD_SCHEMA.into(),
                task: task.clone(),
                thread: None,
                project: "checkout".into(),
                checkout: checkout.display().to_string(),
                worktree: worktree.display().to_string(),
                base: base.clone(),
                turns: Vec::new(),
                ends: Default::default(),
                requested: None,
                shape: Default::default(),
                hooks: None,
                archived: None,
            };
            local::save(&store, &record).unwrap();
            let scratch = Self {
                _root: root,
                checkout,
                store,
                worktree,
                task,
                base,
            };
            scratch.apply(Action::Submit {
                intent: TaskIntent {
                    title: "Chat".into(),
                    prompt: "Fix the parser.".into(),
                    workspace: Workspace {
                        path: scratch.worktree.display().to_string(),
                        source_revision: Some(scratch.base.clone()),
                    },
                    configuration: RequestedConfiguration {
                        adapter: "microcoder-repository".into(),
                        model: None,
                    },
                    images: Vec::new(),
                },
            });
            scratch
        }

        fn apply(&self, action: Action) {
            let mut store = Store::open(&self.store).unwrap();
            let revision = store.show(&self.task).ok().map(|task| task.revision);
            let command = Command {
                schema: COMMAND_SCHEMA.into(),
                command_id: format!("c{}", revision.unwrap_or(0)),
                task_id: self.task.clone(),
                expected_revision: revision,
                action,
            };
            store.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
        }

        /// The task ends: stopped before or after its turn.
        fn end(&self) {
            self.apply(Action::Cancel {
                reason: "Stopped by the person who started it.".into(),
            });
        }

        fn record(&self) -> Record {
            local::record(&self.store, &self.task).unwrap()
        }
    }

    #[test]
    fn a_clean_ended_tasks_worktree_goes_and_comes_back_at_its_commit() {
        let scratch = Scratch::new();
        scratch.end();
        assert_eq!(retire(&scratch.store, &scratch.task), Retired::Removed);
        assert!(!scratch.worktree.exists());
        let listed = git(&scratch.checkout, &["worktree", "list", "--porcelain"]);
        assert!(!listed.contains("checkout-task"), "{listed}");
        let archived = scratch.record().archived.unwrap();
        assert_eq!(archived.commit, scratch.base);
        assert_eq!(archived.branch, None);
        // Retiring again does nothing.
        assert_eq!(retire(&scratch.store, &scratch.task), Retired::Absent);

        let record = ensure(&scratch.store, &scratch.task).unwrap().unwrap();
        assert_eq!(record.archived, None);
        assert_eq!(scratch.record().archived, None);
        assert_eq!(git(&scratch.worktree, &["rev-parse", "HEAD"]), scratch.base);
        assert_eq!(
            std::fs::read_to_string(scratch.worktree.join("a.txt")).unwrap(),
            "one\n"
        );
    }

    fn guarded(worktree: &Path) -> bool {
        coder_boundary::source::APPLIES
            && coder_boundary::source::Guard::for_worktree(worktree)
                .ok()
                .flatten()
                .is_some_and(|guard| guard.enforceable().is_ok())
    }

    #[test]
    fn a_failing_teardown_keeps_the_worktree_and_a_restore_sets_it_up_again() {
        let scratch =
            Scratch::with_hooks(Some(r#"{"setup": "touch ready", "teardown": "exit 4"}"#));
        if !guarded(&scratch.worktree) {
            eprintln!("skipped: the source guard can't be enforced here");
            return;
        }
        scratch.end();
        let kept = retire(&scratch.store, &scratch.task);
        assert!(
            matches!(&kept, Retired::Kept(why) if why.starts_with("its teardown failed")),
            "{kept:?}"
        );
        assert!(scratch.worktree.exists());

        let scratch = Scratch::with_hooks(Some(r#"{"setup": "touch ready", "teardown": "true"}"#));
        scratch.end();
        assert_eq!(retire(&scratch.store, &scratch.task), Retired::Removed);
        let record = ensure(&scratch.store, &scratch.task).unwrap().unwrap();
        assert!(scratch.worktree.join("ready").exists());
        assert_eq!(record.hooks.unwrap().commands, 1);
    }

    #[test]
    fn a_landed_commit_on_the_remote_lets_the_worktree_go() {
        let scratch = Scratch::new();
        std::fs::write(scratch.worktree.join("a.txt"), "two\n").unwrap();
        git(&scratch.worktree, &["commit", "-q", "-am", "fix"]);
        let landed = git(&scratch.worktree, &["rev-parse", "HEAD"]);
        git(&scratch.worktree, &["push", "-q", "origin", "HEAD:main"]);
        git(&scratch.worktree, &["fetch", "-q", "origin"]);
        scratch.end();
        assert_eq!(retire(&scratch.store, &scratch.task), Retired::Removed);
        assert_eq!(scratch.record().archived.unwrap().commit, landed);
    }

    #[test]
    fn a_guarded_push_tracks_the_relative_origin_and_retires_without_fetch() {
        let scratch = Scratch::new();
        git(
            &scratch.checkout,
            &["remote", "set-url", "origin", "../remote.git"],
        );
        git(&scratch.checkout, &["pack-refs", "--all"]);
        let guard = coder_boundary::source::Guard::for_worktree(&scratch.worktree)
            .unwrap()
            .unwrap();
        let enforce = guarded(&scratch.worktree) && !coder_boundary::privacy::sandboxed();
        let allowed: Vec<_> = guard.allowed().collect();
        assert!(allowed.contains(&scratch.checkout.join(".git/refs/remotes").as_path()));
        assert!(allowed.contains(&scratch.checkout.join(".git/logs/refs/remotes").as_path()));
        assert!(!allowed.contains(&scratch.checkout.join(".git/refs/heads").as_path()));
        let before = git(&scratch.checkout, &["rev-parse", "main"]);
        let index = std::fs::read(scratch.checkout.join(".git/index")).unwrap();
        let mut command = if enforce {
            guard.command("/bin/sh", &[&scratch.worktree])
        } else {
            eprintln!(
                "nested source guard unavailable; checking relative push and retirement without nesting"
            );
            std::process::Command::new("/bin/sh")
        };
        let output = command
            .args([
                "-c",
                "printf 'fixed\\n' > a.txt && git commit -qam fix && git push origin HEAD:main",
            ])
            .current_dir(&scratch.worktree)
            .envs(guard.environment())
            .envs(local::remote_override_environment(&scratch.worktree, 0))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("cannot update"));
        let landed = git(&scratch.worktree, &["rev-parse", "HEAD"]);
        assert_eq!(
            git(&scratch.checkout, &["rev-parse", "origin/main"]),
            landed
        );
        assert_eq!(
            git(
                &scratch.checkout.parent().unwrap().join("remote.git"),
                &["rev-parse", "main"]
            ),
            landed
        );
        assert_eq!(git(&scratch.checkout, &["rev-parse", "main"]), before);
        assert_eq!(
            std::fs::read(scratch.checkout.join(".git/index")).unwrap(),
            index
        );
        assert_eq!(
            std::fs::read_to_string(scratch.checkout.join("a.txt")).unwrap(),
            "one\n"
        );
        assert_eq!(
            local::pushed_destination(&scratch.record()).as_deref(),
            Some("origin/main")
        );
        scratch.end();
        assert_eq!(retire(&scratch.store, &scratch.task), Retired::Removed);
        assert!(!scratch.worktree.exists());
        assert_eq!(
            local::pushed_destination(&scratch.record()).as_deref(),
            Some("origin/main")
        );
        assert_eq!(scratch.record().base, scratch.base);
        assert_eq!(scratch.record().archived.unwrap().commit, landed);
        ensure(&scratch.store, &scratch.task).unwrap();
        assert_eq!(git(&scratch.worktree, &["rev-parse", "HEAD"]), landed);
    }

    #[test]
    fn unsaved_work_keeps_the_worktree() {
        // Uncommitted changes.
        let scratch = Scratch::new();
        scratch.end();
        std::fs::write(scratch.worktree.join("a.txt"), "changed\n").unwrap();
        assert_eq!(
            retire(&scratch.store, &scratch.task),
            Retired::Kept("uncommitted changes".into())
        );
        assert!(scratch.worktree.join("a.txt").exists());
        assert_eq!(scratch.record().archived, None);
        assert_eq!(local::pushed_destination(&scratch.record()), None);

        // An ignored file that is not a build cache.
        let scratch = Scratch::new();
        scratch.end();
        std::fs::write(scratch.worktree.join("secret.env"), "KEY=1\n").unwrap();
        let kept = retire(&scratch.store, &scratch.task);
        assert!(
            matches!(&kept, Retired::Kept(why) if why.contains("secret.env")),
            "{kept:?}"
        );

        // A build cache alone does not keep it.
        let scratch = Scratch::new();
        scratch.end();
        std::fs::create_dir_all(scratch.worktree.join("target/debug")).unwrap();
        std::fs::write(scratch.worktree.join("target/debug/x"), "x").unwrap();
        assert_eq!(retire(&scratch.store, &scratch.task), Retired::Removed);

        // A commit on no remote.
        let scratch = Scratch::new();
        scratch.end();
        std::fs::write(scratch.worktree.join("a.txt"), "local\n").unwrap();
        git(&scratch.worktree, &["commit", "-q", "-am", "local only"]);
        assert_eq!(
            retire(&scratch.store, &scratch.task),
            Retired::Kept("commits not on any remote".into())
        );
        assert!(scratch.worktree.exists());
        assert_eq!(local::pushed_destination(&scratch.record()), None);
    }

    #[test]
    fn a_task_that_can_still_go_on_keeps_its_worktree() {
        // Queued: its turn has not ended.
        let scratch = Scratch::new();
        assert_eq!(retire(&scratch.store, &scratch.task), Retired::NotEnded);
        assert!(scratch.worktree.exists());

        // Ended, but its issue flow is still checking and landing it.
        scratch.end();
        let flow = super::super::issue_run::Flow {
            schema: super::super::issue_run::FLOW_SCHEMA.into(),
            task: scratch.task.clone(),
            link: openagents_chat::coder_events::IssueLink {
                repository: "o/r".into(),
                number: 1,
                url: "https://github.com/o/r/issues/1".into(),
                title: "Fix".into(),
                outcome: "landed".into(),
                commits: Vec::new(),
                pull_request: None,
                closed: true,
            },
            notes: Vec::new(),
            finished: false,
            process_id: None,
            closing: String::new(),
            files: None,
        };
        super::super::issue_run::save(&scratch.store, &flow).unwrap();
        assert_eq!(retire(&scratch.store, &scratch.task), Retired::NotEnded);
        assert!(scratch.worktree.exists());

        let finished = super::super::issue_run::Flow {
            finished: true,
            ..flow
        };
        super::super::issue_run::save(&scratch.store, &finished).unwrap();
        assert_eq!(retire(&scratch.store, &scratch.task), Retired::Removed);
    }

    #[test]
    fn a_published_change_lets_the_worktree_go_and_comes_back_with_it() {
        let scratch = Scratch::new();
        std::fs::write(scratch.worktree.join("a.txt"), "published\n").unwrap();
        std::fs::write(scratch.worktree.join("new.txt"), "new\n").unwrap();
        // The publication commits the worktree's content without touching
        // the worktree, and pushes it to a branch of its own.
        let index = scratch.store.join("publish.index");
        let with_index = |args: &[&str]| {
            let output = local::git()
                .arg("-C")
                .arg(&scratch.worktree)
                .env("GIT_INDEX_FILE", &index)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success());
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        };
        with_index(&["read-tree", "HEAD"]);
        with_index(&["add", "-A"]);
        let tree = with_index(&["write-tree"]);
        let commit = git(
            &scratch.worktree,
            &["commit-tree", &tree, "-p", "HEAD", "-m", "published"],
        );
        scratch.end();
        // Not yet on a remote: kept.
        let ledger = |state: &str| {
            serde_json::json!({
                "schema": super::super::publish::PUBLISH_SCHEMA,
                "task": scratch.task,
                "publications": [{
                    "operation": "0".repeat(64),
                    "task": scratch.task,
                    "base": scratch.base,
                    "head_commit": scratch.base,
                    "head": tree,
                    "landing": "draft_pull_request",
                    "state": state,
                    "branch": "coder/x",
                    "commit": commit,
                    "note": "",
                }],
            })
        };
        std::fs::write(
            scratch
                .store
                .join("local")
                .join(format!("{}.publish.json", scratch.task)),
            serde_json::to_vec(&ledger("pushed")).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            retire(&scratch.store, &scratch.task),
            Retired::Kept(_)
        ));
        assert!(scratch.worktree.exists());

        git(
            &scratch.worktree,
            &[
                "push",
                "-q",
                "origin",
                &format!("{commit}:refs/heads/coder/x"),
            ],
        );
        git(&scratch.worktree, &["fetch", "-q", "origin"]);
        // A change after the publication keeps it.
        std::fs::write(scratch.worktree.join("later.txt"), "later\n").unwrap();
        assert_eq!(
            retire(&scratch.store, &scratch.task),
            Retired::Kept("uncommitted changes since it was published".into())
        );
        std::fs::remove_file(scratch.worktree.join("later.txt")).unwrap();

        assert_eq!(retire(&scratch.store, &scratch.task), Retired::Removed);
        assert!(!scratch.worktree.exists());
        assert_eq!(scratch.record().archived.unwrap().commit, commit);
        ensure(&scratch.store, &scratch.task).unwrap();
        assert_eq!(
            std::fs::read_to_string(scratch.worktree.join("new.txt")).unwrap(),
            "new\n"
        );
        assert_eq!(
            std::fs::read_to_string(scratch.worktree.join("a.txt")).unwrap(),
            "published\n"
        );
    }
}
